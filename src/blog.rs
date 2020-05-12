use crate::{Post, FOOTER_FILE_NAME, HEADER_FILE_NAME};

use std::error::Error;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::Path;

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

    pub fn generate<P: AsRef<Path>>(&self, root: P) -> Result<(), Box<dyn Error>> {
        for post in self.posts.iter() {
            // TODO override name with metadata
            let post_file_stem = post.source.file_stem().expect("Post must have filename");
            let post_dir = root.as_ref().join(post_file_stem);
            if post_dir.exists() {
                fs::remove_dir_all(&post_dir)?;
            }
            fs::create_dir(&post_dir)?;

            let post_path = post_dir.join("index.html");
            let mut out = BufWriter::new(fs::File::create(post_path)?);
            write!(out, "{}", HTML_START)?;
            if let Some(header) = &self.header {
                write!(out, "{}", header)?;
            }
            post.write_html(&mut out)?;
            if let Some(footer) = &self.footer {
                write!(out, "{}", footer)?;
            }
            write!(out, "{}", HTML_END)?;
            drop(out);

            for asset in post.assets.iter() {
                let asset_name = asset.file_name().expect("Asset must have file name");
                let dest_path = post_dir.join(asset_name);
                fs::copy(asset, dest_path).expect("File copy failed");
            }
        }

        Ok(())
    }
}
