use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use chrono::offset::Local;
use chrono::{Date, NaiveDate, NaiveDateTime, TimeZone};

/// Represents a Markdown Post that will be converted into HTML.
#[derive(Debug, Clone)]
pub struct Post {
    /// Source file path.
    pub path: PathBuf,
    /// Markdown content with the metadata removed.
    pub markdown: String,
    /// Metadata key-value pairs extracted from the file.
    pub meta: HashMap<String, String>,
    /// Post's title (from the metadata, first heading, or file name).
    pub title: String,
    /// Post's date (from the metadata or filesystem creation date).
    pub date: Date<Local>,
    /// Post's last-modified date (from the metadata or filesystem modified date).
    pub updated: Date<Local>,
    /// Post's category (from the metadata).
    pub category: String,
    /// Post's tags (from the metadata).
    pub tags: Vec<String>,
    /// Post's template (from the metadata).
    pub template: Option<PathBuf>,
    /// Post's absolute URI within a root.
    pub uri: String,
}

impl Post {
    /// Parse a markdown file into a `Post`.
    pub fn new(root: &PathBuf, path: PathBuf) -> io::Result<Self> {
        // UTF-8 BOM becomes zero-width non-breaking space, which `trim()` won't remove,
        // but if we leave it there then metadata loading will break and not recognise
        // where the meta code block starts correctly.
        //
        // Remove it here to avoid such issue (allocating only if needed).
        let mut markdown = fs::read_to_string(&path)?.replace("\u{FEFF}", "");

        let mut meta = HashMap::new();
        if let Some((Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))), start_range)) =
            Parser::new(&markdown).into_offset_iter().next()
        {
            if lang.as_ref() == "meta" {
                meta.extend(markdown[start_range.clone()].lines().filter_map(|line| {
                    let mut kv = line.splitn(2, '=');
                    kv.next()
                        .zip(kv.next())
                        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
                }));
                markdown.replace_range(start_range, "");
            }
        }

        let title = meta
            .get("title")
            .cloned()
            .or_else(|| {
                let mut wait_title = false;
                Parser::new(&markdown).find_map(|event| {
                    match event {
                        Event::Start(Tag::Heading(1)) => wait_title = true,
                        Event::Text(s) if wait_title => {
                            return Some(s.to_string());
                        }
                        _ => {}
                    }
                    None
                })
            })
            .unwrap_or_else(|| {
                path.file_name()
                    .unwrap()
                    .to_str()
                    .expect("bad md file name")
                    .to_owned()
            });

        let metadata = fs::metadata(&path)?;
        let date = meta
            .get("date")
            .and_then(|date| NaiveDate::parse_from_str(date, crate::DATE_FMT).ok())
            .or_else(|| {
                metadata
                    .created()
                    .ok()
                    .and_then(|date| date.duration_since(UNIX_EPOCH).ok())
                    .map(|duration| {
                        NaiveDateTime::from_timestamp(
                            duration.as_secs() as i64,
                            duration.subsec_nanos(),
                        )
                        .date()
                    })
            })
            .and_then(|date| Local.from_local_date(&date).latest())
            .unwrap_or_else(|| Local::now().date());

        let updated = meta
            .get("updated")
            .and_then(|date| NaiveDate::parse_from_str(date, crate::DATE_FMT).ok())
            .or_else(|| {
                metadata
                    .modified()
                    .ok()
                    .and_then(|date| date.duration_since(UNIX_EPOCH).ok())
                    .map(|duration| {
                        NaiveDateTime::from_timestamp(
                            duration.as_secs() as i64,
                            duration.subsec_nanos(),
                        )
                        .date()
                    })
            })
            .and_then(|date| Local.from_local_date(&date).latest())
            .unwrap_or(date);

        let category = meta.get("category").cloned().unwrap_or_else(|| {
            path.parent()
                .expect("post file had no parent")
                .file_name()
                .expect("post parent had no name")
                .to_str()
                .expect("post parent had non-utf8 name")
                .to_owned()
        });

        let tags = meta
            .get("tags")
            .map(|tags| tags.split(',').map(|s| s.trim().to_owned()).collect())
            .unwrap_or_else(Vec::new);

        let template = meta
            .get("template")
            .map(|s| crate::utils::get_abs_path(root, Some(&path), s));

        let uri = crate::utils::replace_root(
            &root.to_str().unwrap().to_owned(),
            &String::new(),
            &path.with_extension("html").to_str().unwrap().to_owned(),
        )
        .to_str()
        .unwrap()
        .replace(std::path::MAIN_SEPARATOR, "/");

        Ok(Self {
            path,
            markdown,
            meta,
            title,
            date,
            updated,
            category,
            tags,
            template,
            uri,
        })
    }
}

// TODO add back old Post tests?
