use crate::html;
use crate::FOLDER_POST_NAME;

use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag};
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

use chrono::offset::Local;
use chrono::{Date, NaiveDate, TimeZone};

#[derive(Debug)]
pub struct Post {
    pub markdown: String,
    /// The name that will become part of the post's URL
    pub path: OsString,
    pub title: String,
    pub modified: Date<Local>,
    pub created: Date<Local>,
    pub assets: Vec<PathBuf>,
}

#[derive(Debug)]
struct Metadata {
    title: Option<String>,
    path: OsString,
    created: Date<Local>,
    modified: Date<Local>,
}

impl Metadata {
    fn update_from_contents(&mut self, contents: &str) -> Option<Range<usize>> {
        #[derive(Debug)]
        enum State {
            Idle,
            WaitTitle,
            WaitMeta(usize),
            WaitEndMeta(usize),
        }

        let mut remove_range = None;

        Parser::new(contents)
            .into_offset_iter()
            .fold(State::Idle, |state, (event, range)| match event {
                Event::Start(Tag::Heading(1)) => State::WaitTitle,
                Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang)))
                    if lang.as_ref() == "meta" =>
                {
                    State::WaitMeta(range.start)
                }
                Event::Text(string) => match state {
                    State::WaitTitle if self.title.is_none() => {
                        self.title = Some(string.to_string());
                        State::Idle
                    }
                    State::WaitMeta(range_start) => {
                        self.update_from_meta_contents(&string);
                        State::WaitEndMeta(range_start)
                    }
                    _ => State::Idle,
                },
                Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(lang)))
                    if lang.as_ref() == "meta" =>
                {
                    match state {
                        State::WaitEndMeta(range_start) => {
                            remove_range = Some(range_start..range.end);
                            State::Idle
                        }
                        _ => panic!(format!("Invalid state reached {:?}", state)),
                    }
                }
                _ => state,
            });

        remove_range
    }

    fn update_from_meta_contents(&mut self, contents: &str) {
        contents
            .split('\n')
            .filter(|line| !line.is_empty())
            .for_each(|line| {
                let mut kv = line.splitn(2, ':');
                let key = kv.next().unwrap();
                let value = kv
                    .next()
                    .expect(&format!("Meta line \"{}\" should contain a colon", line));

                match key.to_lowercase().as_ref() {
                    "title" => self.title = Some(value.trim().to_string()),
                    "path" => self.path = value.trim().into(),
                    "created" => {
                        self.created = parse_date(value)
                            .expect(&format!("Invalid `created` override \"{}\"", value));
                    }
                    "modified" => {
                        self.modified = parse_date(value)
                            .expect(&format!("Invalid `modified` override \"{}\"", value));
                    }
                    _ => {
                        eprintln!(
                            "Unexpected meta override key \"{}\" in post {}, ignoring.",
                            key,
                            self.path.to_string_lossy()
                        );
                    }
                }
            });
    }
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
            content,
            assets,
            Metadata {
                title: None,
                path: path
                    .as_ref()
                    .file_stem()
                    .expect("Post file must have stem")
                    .into(),
                modified,
                created,
            },
        ))
    }

    /// Partially parses markdown to apply meta overrides
    fn from_sources(mut markdown: String, assets: Vec<PathBuf>, mut meta: Metadata) -> Self {
        if let Some(remove_range) = meta.update_from_contents(&markdown) {
            markdown.replace_range(remove_range, "");
        }

        Post {
            markdown,
            path: meta.path,
            title: meta.title.unwrap_or_else(|| "(no title)".to_string()),
            modified: meta.modified,
            created: meta.created,
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
        let markdown = "# My header\n\
        My text goes here...\n\
        More text after that.";
        let assets = vec![];
        let meta = Metadata {
            title: None,
            path: "test".into(),
            created: Local::today(),
            modified: Local::today(),
        };

        let post = Post::from_sources(markdown.into(), assets, meta);

        assert_eq!(post.markdown, markdown);
        assert_eq!(post.title, "My header");
    }

    /// Check that the meta block is extracted from the Post's markdown, and
    /// that properties can be overridden by it.
    #[test]
    fn meta_block_applied() {
        let content = "```meta
title: Overridden Title
path: custom_path
modified: 2020-05-05
```
# My blog post with a long title to be overridden

Some words.";
        let assets = vec![];
        let date = TimeZone::ymd(&Local, 1999, 12, 01);
        let meta = Metadata {
            title: None,
            path: "test_post".into(),
            created: date.clone(),
            modified: date.clone(),
        };

        let post = Post::from_sources(content.into(), assets, meta);

        assert_eq!(
            post.markdown,
            concat!(
                "\n# My blog post with a long title to be overridden\n\n",
                "Some words."
            )
        );
        assert_eq!(post.title, "Overridden Title");
        assert_eq!(post.path, "custom_path");
        assert_eq!(post.created, date.clone());

        let date = Local.ymd(2020, 05, 05);
        assert_eq!(post.modified, date.clone());
    }
}
