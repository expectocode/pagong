use crate::FOLDER_POST_NAME;

use pulldown_cmark::{html, Event, Parser, Tag};
use std::error::Error;
use std::ffi::OsStr;
use std::fs;
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

impl Post {
    pub fn from_source_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
        let metadata = fs::metadata(path.as_ref())?;
        let post_path = if metadata.is_file() {
            path.as_ref().to_path_buf()
        } else if metadata.is_dir() {
            path.as_ref().join(FOLDER_POST_NAME)
        } else {
            unreachable!("Followed symlink is not file or directory");
        };

        let content = fs::read_to_string(&post_path)?;

        let post_metadata = post_path.metadata()?;
        let created = post_metadata.created()?.into();
        let modified = post_metadata.modified()?.into();

        let mut assets = vec![];
        if metadata.is_dir() {
            for child in fs::read_dir(path.as_ref())? {
                let child = child?;
                if child.path().extension() != Some(&OsStr::new("md")) {
                    // don't add .md files as assets
                    assets.push(child.path());
                }
            }
        }

        let mut title = None;
        let mut wait_title = false;
        for event in Parser::new(&content) {
            match event {
                Event::Start(Tag::Heading(1)) => wait_title = true,
                Event::Text(string) if wait_title => {
                    title = Some(string.to_string());
                    break;
                }
                _ => {}
            }
        }

        Ok(Post {
            source: path.as_ref().to_path_buf(),
            markdown: content,
            title: title.unwrap_or_else(|| "(no title)".to_string()),
            modified,
            created,
            assets,
        })
    }

    pub fn push_html(&self, out: &mut String) {
        let parser = Parser::new(&self.markdown);
        html::push_html(out, parser);
    }
}
