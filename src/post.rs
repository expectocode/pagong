use crate::FOLDER_POST_NAME;

use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use chrono::offset::Local;
use chrono::DateTime;

#[derive(Debug)]
pub struct Post {
    pub source: PathBuf,
    pub markdown: String,
    pub title: String,
    pub modified: DateTime<Local>,
    pub created: DateTime<Local>,
    pub assets: Vec<PathBuf>,
}

fn get_content(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(content)
}

impl Post {
    pub fn from_source_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
        let metadata = fs::metadata(path)?;
        let post_path = if metadata.is_file() {
            path.as_ref().to_path_buf()
        } else if metadata.is_dir() {
            path.as_ref().join(FOLDER_POST_NAME)
        } else {
            unreachable!("Followed symlink is not file or directory");
        };

        let content = get_content(&post_path)?;

        let post_metadata = post_path.metadata()?;
        let created = post_metadata.created()?.into();
        let modified = post_metadata.modified()?.into();

        let mut assets = vec![];
        if metadata.is_dir() {
            for child in fs::read_dir(path)? {
                let child = child?;
                if child.path().extension() != Some(&OsStr::new("md")) {
                    // don't add .md files as assets
                    assets.push(child.path());
                }
            }
        }

        Ok(Post {
            source: todo!(),
            markdown: content,
            title: "Title".into(), // TODO
            modified,
            created,
            assets,
        })
    }
}
