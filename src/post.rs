use crate::html;
use crate::FOLDER_POST_NAME;

use pulldown_cmark::{Event, Parser, Tag};
use std::collections::HashMap;
use std::collections::VecDeque;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::offset::Local;
use chrono::DateTime;

#[derive(Debug)]
pub struct Post {
    pub markdown: String,
    pub meta: HashMap<String, String>,
    /// The name that will become part of the post's URL
    pub path: OsString,
    pub title: String,
    pub modified: DateTime<Local>,
    pub created: DateTime<Local>,
    pub assets: Vec<PathBuf>,
}

impl Post {
    /// Construct a post from a standalone title.md or a title/ directory
    /// containing a post.md and optional assets. Performs I/O.
    pub fn from_source_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
        let post_path = if path.as_ref().is_file() {
            path.as_ref().to_path_buf()
        } else if path.as_ref().is_dir() {
            path.as_ref().join(FOLDER_POST_NAME)
        } else {
            unreachable!("Followed symlink is not file or directory");
        };

        let content = fs::read_to_string(&post_path)?;

        let post_metadata = post_path.metadata()?;
        let created = post_metadata.created()?.into();
        let modified = post_metadata.modified()?.into();

        let mut assets = vec![];
        if path.as_ref().is_dir() {
            for child in fs::read_dir(path.as_ref())? {
                let child = child?;
                if child.path().extension() != Some(&OsStr::new("md")) {
                    // don't add .md files as assets
                    assets.push(child.path());
                }
            }
        }

        Ok(Self::from_sources(
            path.as_ref()
                .file_stem()
                .expect("Post file must have stem")
                .into(),
            content,
            assets,
            modified,
            created,
        ))
    }

    /// Partially parses markdown to apply meta overrides
    fn from_sources(
        path: OsString,
        markdown: String,
        assets: Vec<PathBuf>,
        modified: DateTime<Local>,
        created: DateTime<Local>,
    ) -> Self {
        let mut title = None;
        let mut wait_title = false;
        for event in Parser::new(&markdown) {
            match event {
                Event::Start(Tag::Heading(1)) => wait_title = true,
                Event::Text(string) if wait_title => {
                    title = Some(string.to_string());
                    break;
                }
                _ => {}
            }
        }

        // Parse meta overrides
        let mut meta = HashMap::new();
        let mut lines: VecDeque<&str> = markdown.split('\n').collect();

        if lines[0] == "```meta" {
            lines.pop_front();
            loop {
                let line = lines
                    .pop_front()
                    .expect("Unexpected EOF while parsing meta");
                if line == "```" {
                    break;
                }
                let mut kv = line.splitn(2, ':');
                let key = kv.next().unwrap();
                let value = kv
                    .next()
                    .expect(&format!("Meta line \"{}\" should contain a colon", line));
                meta.insert(key.into(), value.into());
            }
            dbg!(&meta);
        }
        // TODO actually parse the meta keys & values

        // Store everything after the meta info on the Post. Re-join with newlines.
        let content = {
            let mut result = String::new();
            let mut iter = lines.into_iter();
            if let Some(line) = iter.next() {
                result.push_str(line);
            }
            for line in iter {
                result.push('\n');
                result.push_str(line);
            }
            result
        };

        Post {
            path,
            markdown: content,
            meta,
            title: title.unwrap_or_else(|| "(no title)".to_string()),
            modified,
            created,
            assets,
        }
    }

    pub fn write_html(&self, header: &str, footer: &str, out: &mut String) {
        let input = header.to_string() + "\n" + &self.markdown + "\n" + footer;
        let parser = Parser::new(&input); // TODO options
        html::push_html(out, parser);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::offset::Local;

    /// Check that the title is extracted from the markdown, and that the content
    /// remains intact.
    #[test]
    fn markdown_title_extracted() {
        let path = "test".into();
        let markdown = "# My header\n\
        My text goes here...\n\
        More text after that.";
        let created = Local::now();
        let modified = created.clone();
        let assets = vec![];

        let post = Post::from_sources(path, markdown.into(), assets, modified, created);

        assert_eq!(post.markdown, markdown);
        assert_eq!(post.title, "My header");
    }
}
