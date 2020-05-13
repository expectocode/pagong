use crate::{Post, FOOTER_FILE_NAME, HEADER_FILE_NAME};

// TODO we don't handle title and other metadata like tags
// TODO if we want to do this proper we should not put header inside main
const HTML_START: &str = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8" />
</head>
<body>
    <main>
"#;

const HTML_END: &str = r#"    </main>
</body>
"#;

#[derive(Debug)]
pub struct Blog {
    pub posts: Vec<Post>,
    pub header: Option<String>,
    pub footer: Option<String>,
}

#[derive(Debug)]
pub enum FsAction {
    Copy {
        source: PathBuf,
        dest: PathBuf,
    },
    DeleteDir {
        path: PathBuf,
        not_exists_ok: bool,
        recursive: bool,
    },
    CreateDir {
        path: PathBuf,
        exists_ok: bool,
    },

    /// Creates file if it does not exist, overwrites if it does exist.
    WriteFile {
        path: PathBuf,
        content: String,
    },
}
use FsAction::*;

impl Blog {
    pub fn from_source_dir<P: AsRef<Path>>(root: P) -> Result<Self, Box<dyn Error>> {
        let mut posts = vec![];
        let mut header = None;
        let mut footer = None;

        for child in fs::read_dir(root)? {
            let child = child?;
            let path = child.path();

            if let Some(name) = path.file_name() {
                if name == HEADER_FILE_NAME {
                    header = Some(fs::read_to_string(path)?);
                    continue;
                } else if name == FOOTER_FILE_NAME {
                    footer = Some(fs::read_to_string(path)?);
                    continue;
                }
            }

            posts.push(Post::from_source_file(path)?);
        }

        Ok(Self {
            posts,
            header,
            footer,
        })
    }

    pub fn generate_actions<P: AsRef<Path>>(&self, root: P) -> Vec<FsAction> {
        let mut actions = vec![];

        for post in self.posts.iter() {
            // TODO override name with metadata
            let post_file_stem = post.source.file_stem().expect("Post must have filename");
            let post_dir = root.as_ref().join(post_file_stem);
            actions.push(DeleteDir {
                path: post_dir.clone(),
                not_exists_ok: true,
                recursive: true,
            });
            actions.push(CreateDir {
                path: post_dir.clone(),
                exists_ok: false,
            });

            let post_path = post_dir.join("index.html");

            let mut file_content = String::new();
            file_content.push_str(HTML_START);
            file_content.push_str(&self.header.as_ref().unwrap_or(&String::new()));
            post.push_html(&mut file_content);
            file_content.push_str(&self.footer.as_ref().unwrap_or(&String::new()));
            file_content.push_str(HTML_END);

            actions.push(WriteFile {
                path: post_path,
                content: file_content,
            });

            for asset in post.assets.iter() {
                let asset_name = asset.file_name().expect("Asset must have file name");
                let dest_path = post_dir.join(asset_name);
                actions.push(Copy {
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

    #[test]
    fn standalone_file_post_generated() {
        let blog = Blog {
            posts: vec![Post {
                source: "/path/to/content/test_post.md".into(),
                markdown: "A test post".into(),
                title: "A test post title".into(),
                modified: Local::now(),
                created: Local::now(),
                assets: vec![],
            }],
            header: None,
            footer: None,
        };

        let actions = blog.generate_actions("dist");

        assert_eq!(actions.len(), 3);
        assert!(matches!(&actions[0] ,
           DeleteDir {
            path,
            not_exists_ok: true,
            recursive: true
           } if path == Path::new("dist/test_post")
        ));

        assert!(matches!(&actions[1] ,
           CreateDir {
            path,
            ..
           } if path == Path::new("dist/test_post")
        ));

        assert!(matches!(&actions[2] ,
           WriteFile {
            path,
            content
           } if path == Path::new("dist/test_post/index.html") && content.contains("A test poop")
        ));
    }
}
