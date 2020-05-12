use crate::{Post, FOOTER_FILE_NAME, HEADER_FILE_NAME};

use std::error::Error;
use std::fs;
use std::path::Path;

#[derive(Debug)]
pub struct Blog {
    pub posts: Vec<Post>,
    pub header: String,
    pub footer: String,
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
}
