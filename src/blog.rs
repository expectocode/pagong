use crate::fs_action::{execute_fs_actions, FsAction};
use crate::{Post, CSS_DIR_NAME, CSS_FILE_NAME, FOOTER_FILE_NAME, HEADER_FILE_NAME};

use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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

        for post in self.posts.iter() {
            // TODO override name with metadata
            let post_file_stem = post.source.file_stem().expect("Post must have filename");
            let post_dir = root.as_ref().join(post_file_stem);
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

            let css = format!("../css/{}", CSS_FILE_NAME);
            let header = self.header.as_ref().map(|s| s.as_str()).unwrap_or("");
            let footer = self.footer.as_ref().map(|s| s.as_str()).unwrap_or("");

            actions.push(FsAction::WriteFile {
                path: post_path,
                content: generate_html(post, header, footer, &css),
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

        actions
    }
}

#[cfg(test)]
mod tests {
    // If FsAction stuff gets more complex, it might be worth implementing a mock
    // executor so that we can test for results rather than individual actions.
    use super::*;
    use chrono::offset::Local;
    use std::collections::HashMap;

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
                source: "/path/to/content/test_post.md".into(),
                markdown: "A test post".into(),
                meta: HashMap::new(),
                title: "A test post title".into(),
                modified: Local::now(),
                created: Local::now(),
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
