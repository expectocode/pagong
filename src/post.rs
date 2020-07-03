use crate::{html, AppError, FOLDER_POST_NAME};

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::offset::Local;
use chrono::{Date, NaiveDate, TimeZone};

#[derive(Debug, Clone)]
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
        let mut parser = Parser::new(contents).into_offset_iter();
        let mut remove_range = None;

        let first = parser.next();
        if let Some((Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))), start_range)) =
            first
        {
            if lang.as_ref() == "meta" {
                remove_range = Some(start_range.clone());
                self.update_from_meta_contents(&contents[start_range]);
            }
        }

        if self.title.is_none() {
            // Extract first header as title
            let mut wait_title = false;
            for event in Parser::new(&contents) {
                match event {
                    Event::Start(Tag::Heading(1)) => wait_title = true,
                    Event::Text(s) if wait_title => {
                        self.title = Some(s.to_string());
                        break;
                    }
                    _ => {}
                }
            }
        }

        remove_range
    }

    fn update_from_meta_contents(&mut self, contents: &str) {
        for line in contents
            .split('\n')
            .skip(1) // ```meta
            .filter(|line| !line.is_empty() && line.trim() != "```")
        {
            let mut kv = line.splitn(2, ':');
            let key = kv.next().unwrap();
            let value = if let Some(v) = kv.next() {
                v
            } else {
                eprintln!("Ignoring meta override line {:?} in post {:?} because it does not have a value", line, self.path);
                continue;
            };

            match key.to_lowercase().as_ref() {
                "title" => self.title = Some(value.trim().to_string()),
                "path" => self.path = value.trim().into(),
                "created" | "published" => match parse_date(value) {
                    Ok(date) => self.created = date,
                    Err(_) => eprintln!("Invalid {:?} override value for {:?} in post {:?} because the format was not YYYY-mm-dd", value, key, self.path),
                },
                "modified" | "updated" => match parse_date(value) {
                    Ok(date) => self.modified = date,
                    Err(_) => eprintln!("Invalid {:?} override value for {:?} in post {:?} because the format was not YYYY-mm-dd", value, key, self.path),
                },
                _ => {
                    eprintln!(
                        "Unexpected meta override key {:?} in post {:?}, ignoring.",
                        key,
                        self.path
                    );
                }
            }
        }
    }
}

impl Post {
    /// Construct a post from a standalone title.md or a title/ directory
    /// containing a post.md and optional assets. Performs I/O.
    pub fn from_source_file<P: AsRef<Path>>(path: P) -> Result<Self, AppError> {
        let post_path = if path.as_ref().is_file() {
            path.as_ref().to_path_buf()
        } else if path.as_ref().is_dir() {
            path.as_ref().join(FOLDER_POST_NAME)
        } else {
            unreachable!("Followed symlink is not file or directory");
        };

        let content = fs::read_to_string(&post_path).map_err(|e| AppError::ReadFile {
            source: e,
            path: post_path.clone(),
        })?;

        let post_metadata = post_path.metadata().map_err(|e| AppError::FileMeta {
            source: e,
            path: post_path.clone(),
        })?;
        let created = post_metadata
            .created()
            .unwrap_or_else(|_| SystemTime::now());
        let modified = post_metadata
            .modified()
            .unwrap_or_else(|_| SystemTime::now());

        let created = chrono::DateTime::from(created).date();
        let modified = chrono::DateTime::from(modified).date();

        let mut assets = vec![];
        if path.as_ref().is_dir() {
            for child in fs::read_dir(path.as_ref()).map_err(|e| AppError::ReadDir {
                source: e,
                path: path.as_ref().into(),
            })? {
                let child = child.map_err(|e| AppError::IterDir {
                    source: e,
                    path: path.as_ref().into(),
                })?;
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
        // UTF-8 BOM becomes zero-width non-breaking space, which `trim()` won't remove,
        // but if we leave it there then metadata loading will break and not recognise
        // where the meta code block starts correctly.
        //
        // Remove it here to avoid such issue (allocating only if needed).
        if markdown.contains("\u{FEFF}") {
            markdown = markdown.replace("\u{FEFF}", "");
        }

        if let Some(remove_range) = meta.update_from_contents(&markdown) {
            markdown.replace_range(remove_range, "");
        }

        // Remove leading whitespace
        markdown = markdown.trim_start().into();

        Post {
            markdown,
            path: meta.path,
            title: meta.title.unwrap_or_else(|| "(no title)".to_string()),
            modified: meta.modified,
            created: meta.created,
            assets,
        }
    }

    pub fn generate_summary(&self) -> Option<String> {
        let mut expecting_text = false;
        Parser::new(&self.markdown).find_map(|e| match e {
            Event::Start(Tag::Paragraph) => {
                expecting_text = true;
                None
            }
            Event::Text(text) if expecting_text => Some(text.to_string()),
            _ => None,
        })
    }

    pub fn write_html(&self, header: &str, footer: &str, out: &mut String) {
        let date_format = "%Y-%m-%d";
        let date_divs = if self.modified == self.created {
            format!(
                "<div class=\"date-created-modified\">{}</div>",
                self.created.format(date_format)
            )
        } else {
            format!(
                "<div class=\"date-created-modified\">Created {}<br>
Modified {}</div>",
                self.created.format(date_format),
                self.modified.format(date_format)
            )
        };

        // Insert date after first element (usually the title)
        let options = Options::all();
        let mut parser = Parser::new_ext(&self.markdown, options).into_offset_iter();
        let (_, first_range) = parser.next().expect("Post must have at least one element");
        let main = self.markdown[first_range.clone()].to_string()
            + "\n"
            + &date_divs
            + "\n"
            + &self.markdown[first_range.end..];

        let input = header.to_string() + "\n" + &main + "\n" + footer;

        let parser = Parser::new_ext(&input, options);
        html::push_html(out, parser);
    }
}

/// Parse a string of the form YYYY-MM-DD into a "local" Date
fn parse_date(date: &str) -> chrono::ParseResult<chrono::Date<Local>> {
    // ISO-format has priority
    NaiveDate::parse_from_str(date, "%Y-%m-%dT%H:%M:%S%z")
        .or_else(|_| NaiveDate::parse_from_str(date, "%Y-%m-%d"))
        .map(|date| {
            TimeZone::from_local_date(&Local, &date)
                .latest()
                .expect("There should always be a latest date")
        })
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
                "# My blog post with a long title to be overridden\n\n",
                "Some words."
            )
        );
        assert_eq!(post.title, "Overridden Title");
        assert_eq!(post.path, "custom_path");
        assert_eq!(post.created, date.clone());

        let date = Local.ymd(2020, 05, 05);
        assert_eq!(post.modified, date.clone());
    }

    /// Check that an invalid meta block does not cause the program to panic.
    #[test]
    fn bad_meta_block_wont_crash() {
        let content = "``` meta
title: Bad Meta
```

:)";
        let assets = vec![];
        let date = TimeZone::ymd(&Local, 1999, 12, 01);
        let meta = Metadata {
            title: None,
            path: "test_post".into(),
            created: date.clone(),
            modified: date.clone(),
        };

        let post = Post::from_sources(content.into(), assets, meta);
        assert_eq!(post.title, "Bad Meta");
    }

    /// Check that a meta key with no value does not cause the program to panic.
    #[test]
    fn missing_value_in_meta_block_wont_crash() {
        let content = "```meta
title or not to title
```

:D";
        let assets = vec![];
        let date = TimeZone::ymd(&Local, 1999, 12, 01);
        let meta = Metadata {
            title: None,
            path: "test_post".into(),
            created: date.clone(),
            modified: date.clone(),
        };

        let post = Post::from_sources(content.into(), assets, meta);
        assert_eq!(post.markdown, ":D");
    }

    /// Check that an invalid date does not cause the program to panic.
    #[test]
    fn bad_date_wont_crash() {
        let content = "```meta
created: today lol
```

:-O";
        let assets = vec![];
        let date = TimeZone::ymd(&Local, 1999, 12, 01);
        let meta = Metadata {
            title: None,
            path: "test_post".into(),
            created: date.clone(),
            modified: date.clone(),
        };

        let post = Post::from_sources(content.into(), assets, meta);
        assert_eq!(post.markdown, ":-O");
    }

    /// Check that UTF-8 with BOM does not break meta parsing.
    #[test]
    fn utf8_bom_works_fine() {
        let content = String::from_utf8(
            b"\xEF\xBB\xBF```meta
```

# Boom"
                .iter()
                .copied()
                .collect(),
        )
        .unwrap();
        let assets = vec![];
        let date = TimeZone::ymd(&Local, 1999, 12, 01);
        let meta = Metadata {
            title: None,
            path: "test_post".into(),
            created: date.clone(),
            modified: date.clone(),
        };

        let post = Post::from_sources(content, assets, meta);
        assert_eq!(post.title, "Boom");
    }

    /// Check that the summary is found correctly.
    #[test]
    fn find_no_summary() {
        let content = r#"
# My blog post

```rust
println!("Jokes on you, this is not a summary but 🦀!");
```
"#;
        let assets = vec![];
        let date = TimeZone::ymd(&Local, 1999, 12, 01);
        let meta = Metadata {
            title: None,
            path: "test_post".into(),
            created: date.clone(),
            modified: date.clone(),
        };

        let post = Post::from_sources(content.into(), assets, meta);
        assert_eq!(post.generate_summary(), None);
    }

    /// Check that the summary is found correctly.
    #[test]
    fn find_summary() {
        let content = r#"
# My blog post

```rust
println!("Not quite yet a summary…");
```

This totally summarizes the post.

However, this does not.
"#;
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
            post.generate_summary(),
            Some("This totally summarizes the post.".to_string())
        );
    }
}
