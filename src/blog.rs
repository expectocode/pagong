use crate::fs_action::{execute_fs_actions, FsAction};
use crate::{Post, CSS_DIR_NAME, CSS_FILE_NAME, FOOTER_FILE_NAME, HEADER_FILE_NAME};

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use atom_syndication as atom;

// // TODO we don't handle title and other metadata like tags
// // TODO if we want to do this proper we should not put header inside main

#[derive(Debug)]
pub struct Blog {
    pub posts: Vec<Post>,
    pub css_path: Option<PathBuf>,
    pub header: Option<String>,
    pub footer: Option<String>,
}

fn generate_html(
    title: &str,
    css: &str,
    body_writer: &dyn Fn(&mut String) -> Result<()>,
) -> Result<String> {
    let mut html = String::new();
    html.push_str(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
"#,
    );
    html.push_str(&format!("<title>{}</title>\n", title));
    html.push_str(&format!(r#"<link rel="stylesheet" href="{}">"#, css));
    html.push('\n');
    html.push_str(
        "</head>\n\
                 <body>\n\
                 <main>\n",
    );

    body_writer(&mut html).context(format!("Body of post '{}' could not be written", title))?;

    html.push_str(
        "</main>\n\
                 </body>\n\
                 </html>\n ",
    );

    Ok(html)
}

fn generate_post_html(post: &Post, header: &str, footer: &str, css: &str) -> Result<String> {
    generate_html(&post.title, css, &|html| {
        post.write_html(header, footer, html)
    })
}

impl Blog {
    pub fn from_source_dir<P: AsRef<Path>>(root: P) -> Result<Self> {
        let mut posts = vec![];
        let mut header = None;
        let mut footer = None;
        let mut css_path = None;
        let root = root.as_ref();

        for child in
            fs::read_dir(root).context(format!("Could not read root directory '{:?}'", root))?
        {
            let child = child.context(format!(
                "Could not list contents of root directory '{:?}'",
                root
            ))?;
            let path = child.path();

            if let Some(name) = path.file_name() {
                if name == HEADER_FILE_NAME {
                    header = Some(fs::read_to_string(&path).context(format!(
                        "Could not read contents of header file '{:?}'",
                        path
                    ))?);
                    continue;
                } else if name == FOOTER_FILE_NAME {
                    footer = Some(fs::read_to_string(&path).context(format!(
                        "Could not read contents of footer file '{:?}'",
                        path
                    ))?);
                    continue;
                } else if name == CSS_FILE_NAME {
                    css_path = Some(path.clone());
                    continue;
                }
            }

            let ty = match child.file_type() {
                Ok(ty) => ty,
                Err(_) => continue,
            };

            if ty.is_dir() {
                posts.push(Post::from_source_file(path)?);
            } else if let Some(ext) = path.extension() {
                if let Some(ext) = ext.to_str() {
                    if ext.eq_ignore_ascii_case("md") {
                        posts.push(Post::from_source_file(path)?);
                    }
                }
                // else if it's not valid UTF-8 then it won't match "md" anyway
            }
        }

        Ok(Self {
            posts,
            css_path,
            header,
            footer,
        })
    }

    pub fn generate<P: AsRef<Path>>(&self, root: P) -> Result<()> {
        let actions = self
            .generate_actions(root)
            .context("Could not generate all blog information")?;
        execute_fs_actions(&actions)
    }

    pub fn generate_actions<P: AsRef<Path>>(&self, root: P) -> Result<Vec<FsAction>> {
        let blog_root = "https://expectocode.github.io/pagong".to_string(); // TODO user-provided
        let author_name = "expectocode"; // TODO user-provided
        let blog_title = "pagong"; // TODO user-provided
        let mut actions = vec![];

        // Copy CSS assets
        if let Some(css_source) = &self.css_path {
            let css_path = root.as_ref().join(CSS_DIR_NAME);
            actions.push(FsAction::DeleteDir {
                path: css_path.clone(),
                not_exists_ok: true,
                recursive: true,
            });
            actions.push(FsAction::CreateDir {
                path: css_path.clone(),
                exists_ok: false,
            });
            actions.push(FsAction::Copy {
                source: css_source.clone(),
                dest: css_path.join(CSS_FILE_NAME),
            });
        }

        // Sorting the posts so that the atom feed is correctly ordered.
        // Do iter and collect to work over references and thus avoid cloning.
        let mut sorted_posts: Vec<_> = self.posts.iter().collect();
        sorted_posts.sort_by(|b, a| {
            a.modified
                .partial_cmp(&b.modified)
                .expect("Failed to compare modified dates")
                .then_with(|| {
                    a.created
                        .partial_cmp(&b.created)
                        .expect("Failed to compare created dates")
                        .then_with(|| a.title.cmp(&b.title))
                })
        });

        // Because the atom feed also takes HTML content, generate both the
        // HTML and the feed entries in the same place.
        let mut entries = Vec::with_capacity(self.posts.len());
        for &post in sorted_posts.iter() {
            // TODO override name with metadata
            let post_dir_name = &post.path;
            let post_dir = root.as_ref().join(post_dir_name);
            actions.push(FsAction::DeleteDir {
                path: post_dir.clone(),
                not_exists_ok: true,
                recursive: true,
            });
            actions.push(FsAction::CreateDir {
                path: post_dir.clone(),
                exists_ok: false,
            });

            let post_path = post_dir.join("index.html");

            // TODO this looks like a bad way to handle the path to the css
            let css = format!("../{}/{}", CSS_DIR_NAME, CSS_FILE_NAME);
            let header = self.header.as_ref().map(|s| s.as_str()).unwrap_or("");
            let footer = self.footer.as_ref().map(|s| s.as_str()).unwrap_or("");
            let html = generate_post_html(post, header, footer, &css).context(format!(
                "Could not generate HTML for post '{}', at path {:?}",
                post.title, post.path
            ))?;

            let mut escaped_html = String::with_capacity(html.len());
            crate::escape::escape_html(&mut escaped_html, &html)
                .expect("Escaping HTML in-memory failed");

            entries.push(
                // Additionally, we could add category or other extra information here
                atom::Entry {
                    title: post.title.clone(),
                    // `id` fields on entries are required to be complete URLs.
                    id: PathBuf::from(&blog_root)
                        .join(post_dir_name)
                        .join("index.html")
                        .to_string_lossy()
                        .to_string(),
                    updated: chrono::DateTime::<chrono::FixedOffset>::from(
                        post.modified.and_hms(0, 0, 0),
                    ),
                    published: Some(chrono::DateTime::<chrono::FixedOffset>::from(
                        post.created.and_hms(0, 0, 0),
                    )),
                    summary: post.generate_summary(),
                    content: Some(atom::Content {
                        value: Some(escaped_html),
                        src: None,
                        content_type: Some("html".to_string()),
                    }),
                    authors: vec![atom::Person {
                        name: author_name.into(),
                        ..atom::Person::default()
                    }],
                    ..atom::Entry::default()
                },
            );

            actions.push(FsAction::WriteFile {
                path: post_path,
                content: html,
            });

            for asset in post.assets.iter() {
                let asset_name = asset.file_name().expect("Asset must have file name");
                let dest_path = post_dir.join(asset_name);
                actions.push(FsAction::Copy {
                    source: asset.into(),
                    dest: dest_path,
                });
            }
        }

        // Generate main-page listing
        actions.push(FsAction::WriteFile {
            path: root.as_ref().join("index.html").into(),
            content: generate_html(
                blog_title,
                &format!("{}/{}", CSS_DIR_NAME, CSS_FILE_NAME),
                &|mut html| {
                    html.push_str("<ul>");
                    sorted_posts.iter().for_each(|&post| {
                        html.push_str("<li><a href=\"");
                        crate::escape::escape_href(&mut html, &post.path.to_string_lossy())
                            .expect("Should not fail to escape HREF in-memory");
                        html.push_str("/index.html\">");
                        crate::escape::escape_html(&mut html, &post.title)
                            .expect("Should not fail to escape HTML in-memory");
                        html.push_str("</a></li>");
                    });
                    html.push_str("</ul>");
                    Ok(())
                },
            )?,
        });

        // Generate atom feed
        // Similarly, we could add author, contributor, icon, or logo information here.
        // TODO: It would be nice to automatically test validity against the Atom schema,
        // to ensure the best support by feed readers.
        actions.push(FsAction::WriteFile {
            path: root.as_ref().join("atom.xml").into(),
            content: atom::Feed {
                title: blog_title.into(),
                id: blog_root.clone(),
                updated: if let Some(post) = sorted_posts.get(0) {
                    chrono::DateTime::<chrono::FixedOffset>::from(post.created.and_hms(0, 0, 0))
                } else {
                    chrono::offset::Local::now().into()
                },
                entries,
                links: vec![atom::Link {
                    rel: "self".into(),
                    href: PathBuf::from(&blog_root)
                        .join("atom.xml")
                        .to_string_lossy()
                        .into(),
                    ..atom::Link::default()
                }],
                ..atom::Feed::default()
            }
            .to_string(),
        });

        Ok(actions)
    }
}

#[cfg(test)]
mod tests {
    // If FsAction stuff gets more complex, it might be worth implementing a mock
    // executor so that we can test for results rather than individual actions.
    use super::*;
    use chrono::offset::Local;

    #[test]
    fn css_file_copied() -> Result<()> {
        let source_css_file = Path::new("path/to/content/").join(CSS_FILE_NAME);
        let root = Path::new("dist");
        let gen_css_dir = root.join(CSS_DIR_NAME);

        let blog = Blog {
            posts: vec![],
            css_path: Some(source_css_file.clone()),
            header: None,
            footer: None,
        };

        let actions = blog.generate_actions(root)?;

        assert_eq!(actions.len(), 5);
        assert!(matches!(&actions[0] ,
           FsAction::DeleteDir {
            path,
            not_exists_ok: true,
            recursive: true
           } if path == &gen_css_dir
        ));

        assert!(matches!(&actions[1] ,
          FsAction::CreateDir {
            path,
            ..
           } if path == &gen_css_dir
        ));

        assert!(matches!(&actions[2] ,
           FsAction::Copy{
            source,
            dest,
           } if source == &source_css_file && dest == &gen_css_dir.join(CSS_FILE_NAME)
        ));

        assert!(matches!(&actions[3] ,
            FsAction::WriteFile {
                path,
                content
            } if path == Path::new("dist/index.html") && content.contains("<ul>")
        ));

        assert!(matches!(&actions[4] ,
            FsAction::WriteFile {
                path,
                content
            } if path == Path::new("dist/atom.xml") && content.contains("http://www.w3.org/2005/Atom")
        ));

        Ok(())
    }

    #[test]
    fn standalone_file_post_generated() -> Result<()> {
        let blog = Blog {
            posts: vec![Post {
                path: "test_post".into(),
                markdown: "A test post".into(),
                title: "A test post title".into(),
                modified: Local::today(),
                created: Local::today(),
                assets: vec![],
            }],
            css_path: None,
            header: None,
            footer: None,
        };

        let actions = blog.generate_actions("dist")?;

        assert_eq!(actions.len(), 5);
        assert!(matches!(&actions[0] ,
           FsAction::DeleteDir {
            path,
            not_exists_ok: true,
            recursive: true
           } if path == Path::new("dist/test_post")
        ));

        assert!(matches!(&actions[1] ,
          FsAction::CreateDir {
            path,
            ..
           } if path == Path::new("dist/test_post")
        ));

        assert!(matches!(&actions[2] ,
           FsAction::WriteFile {
            path,
            content
           } if path == Path::new("dist/test_post/index.html") && content.contains("A test post")
        ));

        assert!(matches!(&actions[3] ,
            FsAction::WriteFile {
                path,
                content
            } if path == Path::new("dist/index.html") && content.contains("<ul>")
        ));

        assert!(matches!(&actions[4] ,
            FsAction::WriteFile {
                path,
                content
            } if path == Path::new("dist/atom.xml") && content.contains("http://www.w3.org/2005/Atom")
        ));
        Ok(())
    }
}
