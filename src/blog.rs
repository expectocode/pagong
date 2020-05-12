use crate::{Post, FOOTER_FILE_NAME, HEADER_FILE_NAME};

use pulldown_cmark::{html, Parser};
use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::Path;

#[derive(Debug)]
pub struct Blog {
    pub posts: Vec<Post>,
    pub header: String,
    pub footer: String,
}

fn translate_to_html(header: &str, body: &str, footer: &str) -> String {
    let input: String = header.to_string() + "\n" + body + "\n" + footer;
    let parser = Parser::new(&input); // TODO options

    let mut output = String::new();
    html::push_html(&mut output, parser);

    output
}

impl Blog {
    pub fn from_source_dir<P: AsRef<Path>>(root: P) -> Result<Self, Box<dyn Error>> {
        let mut posts = vec![];
        let mut header = "".to_string();
        let mut footer = "".to_string();

        for child in fs::read_dir(root)? {
            let child = child?;
            let path = child.path();

            if let Some(name) = path.file_name() {
                if name == HEADER_FILE_NAME {
                    header = fs::read_to_string(path)?;
                    continue;
                } else if name == FOOTER_FILE_NAME {
                    footer = fs::read_to_string(path)?;
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
            let mut out = fs::File::create(post_path)?;

            let html = translate_to_html(&self.header, &post.markdown, &self.footer);

            for asset in post.assets.iter() {
                let asset_name = asset.file_name().expect("Asset must have file name");
                let dest_path = post_dir.join(asset_name);
                fs::copy(asset, dest_path).expect("File copy failed");
            }

            out.write(html.as_bytes())?;
        }

        Ok(())
    }
}
