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
use chrono::{Date, NaiveDate, TimeZone};

#[derive(Debug)]
pub struct Post {
    pub markdown: String,
    pub meta: HashMap<String, String>,
    /// The name that will become part of the post's URL
    pub path: OsString,
    pub title: String,
    pub modified: Date<Local>,
    pub created: Date<Local>,
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
        let created = post_metadata.created()?;
        let modified = post_metadata.modified()?;

        let created = chrono::DateTime::from(created).date();
        let modified = chrono::DateTime::from(modified).date();

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
        modified: Date<Local>,
        created: Date<Local>,
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
        let mut meta: HashMap<String, String> = HashMap::new();
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
        }

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

        let mut path = path;
        let mut created = created;
        let mut modified = modified;

        for (key, new_value) in meta.iter() {
            match key.to_lowercase().as_ref() {
                "title" => title = Some(new_value.trim().to_string()),
                "path" => path = new_value.trim().into(),
                "created" => {
                    created = parse_date(new_value)
                        .expect(&format!("Invalid `created` override \"{}\"", new_value));
                }
                "modified" => {
                    modified = parse_date(new_value)
                        .expect(&format!("Invalid `modified` override \"{}\"", new_value));
                }
                _ => {
                    eprintln!(
                        "Unexpected meta override key \"{}\" in post {}, ignoring.",
                        key,
                        path.to_string_lossy()
                    );
                }
            }
        }

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

/// Parse a string of the form YYYY-MM-DD into a "local" Date
fn parse_date(date: &str) -> Option<chrono::Date<Local>> {
    let naive = NaiveDate::parse_from_str(date, "%Y-%m-%d");
    naive
        .ok()
        .map(|date| TimeZone::from_local_date(&Local, &date).latest())
        .expect("Override date should be valid YYYY-MM-DD")
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
        let created = Local::today();
        let modified = created.clone();
        let assets = vec![];

        let post = Post::from_sources(path, markdown.into(), assets, modified, created);

        assert_eq!(post.markdown, markdown);
        assert_eq!(post.title, "My header");
    }

    /// Check that the meta block is extracted from the Post's markdown, and
    /// that properties can be overridden by it.
    #[test]
    fn meta_block_applied() {
        let path: OsString = "test_post".into();
        let content = "```meta
title: Overridden Title
path: custom_path
modified: 2020-05-05
```
# My blog post with a long title to be overridden

Some words.";

        let created = TimeZone::ymd(&Local, 1999, 12, 01);
        let modified = created.clone();
        let assets = vec![];

        let post = Post::from_sources(path, content.into(), assets, modified, created);

        assert_eq!(
            post.markdown,
            concat!(
                "# My blog post with a long title to be overridden\n\n",
                "Some words."
            )
        );
        assert_eq!(post.title, "Overridden Title");
        assert_eq!(post.path, "custom_path");
        assert_eq!(post.modified, Local.ymd(2020, 05, 05));
        assert_eq!(post.created, created);
    }
}
