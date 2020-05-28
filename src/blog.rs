use crate::fs_action::{execute_fs_actions, FsAction};
use crate::{Post, CSS_DIR_NAME, CSS_FILE_NAME, FOOTER_FILE_NAME, HEADER_FILE_NAME};

use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use atom_syndication::{ContentBuilder, EntryBuilder, FeedBuilder};

// // TODO we don't handle title and other metadata like tags
// // TODO if we want to do this proper we should not put header inside main

#[derive(Debug)]
pub struct Blog {
    pub posts: Vec<Post>,
    pub css_path: Option<PathBuf>,
    pub header: Option<String>,
    pub footer: Option<String>,
}

fn generate_html(post: &Post, header: &str, footer: &str, css: &str) -> String {
    let mut html = String::new();
    html.push_str(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
"#,
    );
    html.push_str(&format!("<title>{}</title>\n", post.title));
    html.push_str(&format!(r#"<link rel="stylesheet" href="{}">"#, css));
    html.push('\n');
    html.push_str(
        "</head>\n\
                 <body>\n\
                 <main>\n",
    );

    post.write_html(header, footer, &mut html);

    html.push_str(
        "</main>\n\
                 </body>\n\
                 </html>\n ",
    );

    html
}

impl Blog {
    pub fn from_source_dir<P: AsRef<Path>>(root: P) -> Result<Self, Box<dyn Error>> {
        let mut posts = vec![];
        let mut header = None;
        let mut footer = None;
        let mut css_path = None;

        for child in fs::read_dir(root)? {
            let child = child?;
            let path = child.path();

            if let Some(name) = path.file_name() {
                if name == HEADER_FILE_NAME {
                    header = Some(fs::read_to_string(&path)?);
                    continue;
                } else if name == FOOTER_FILE_NAME {
                    footer = Some(fs::read_to_string(&path)?);
                    continue;
                } else if name == CSS_FILE_NAME {
                    css_path = Some(path.clone());
                    continue;
                }
            }

            posts.push(Post::from_source_file(path)?);
        }

        Ok(Self {
            posts,
            css_path,
            header,
            footer,
        })
    }

    pub fn generate<P: AsRef<Path>>(&self, root: P) -> io::Result<()> {
        let actions = self.generate_actions(root);
        execute_fs_actions(&actions)
    }

    pub fn generate_actions<P: AsRef<Path>>(&self, root: P) -> Vec<FsAction> {
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

            let css = format!("../{}/{}", CSS_DIR_NAME, CSS_FILE_NAME);
            let header = self.header.as_ref().map(|s| s.as_str()).unwrap_or("");
            let footer = self.footer.as_ref().map(|s| s.as_str()).unwrap_or("");
            let html = generate_html(post, header, footer, &css);

            entries.push(
                // Additionally, we could add author or category information here
                EntryBuilder::default()
                    .title(post.title.clone())
                    .id(post_path.to_string_lossy().to_string())
                    .updated(chrono::DateTime::<chrono::FixedOffset>::from(
                        post.modified.and_hms(0, 0, 0),
                    ))
                    .published(chrono::DateTime::<chrono::FixedOffset>::from(
                        post.created.and_hms(0, 0, 0),
                    ))
                    .summary(post.generate_summary())
                    .content(
                        ContentBuilder::default()
                            .value(html.clone())
                            .src(post_path.to_string_lossy().to_string())
                            .content_type("html".to_string())
                            .build()
                            .expect("required content field missing"),
                    )
                    .build()
                    .expect("required entry field missing"),
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

        // Generate atom feed
        // Similarly, we could add author, contributor, icon, or logo information here
        actions.push(FsAction::WriteFile {
            path: root.as_ref().join("atom.xml").into(),
            content: FeedBuilder::default()
                .title("tortuga") // TODO blog title
                .id("tortuga") // TODO blog id?
                .updated(chrono::DateTime::<chrono::FixedOffset>::from(
                    sorted_posts[0].created.and_hms(0, 0, 0),
                ))
                .entries(entries)
                .build()
                .expect("required feed field missing")
                .to_string(),
        });

        actions
    }
}

#[cfg(test)]
mod tests {
    // If FsAction stuff gets more complex, it might be worth implementing a mock
    // executor so that we can test for results rather than individual actions.
    use super::*;
    use chrono::offset::Local;

    #[test]
    fn css_file_copied() {
        let source_css_file = Path::new("path/to/content/").join(CSS_FILE_NAME);
        let root = Path::new("dist");
        let gen_css_dir = root.join(CSS_DIR_NAME);

        let blog = Blog {
            posts: vec![],
            css_path: Some(source_css_file.clone()),
            header: None,
            footer: None,
        };

        let actions = blog.generate_actions(root);

        assert_eq!(actions.len(), 3);
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
    }

    #[test]
    fn standalone_file_post_generated() {
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

        let actions = blog.generate_actions("dist");

        assert_eq!(actions.len(), 3);
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
    }
}
