use crate::config::{
    INCLUDE_RAW_EXTENSIONS, META_KEY_CATEGORY, META_KEY_CREATION_DATE, META_KEY_MODIFIED_DATE,
    META_KEY_TAGS, META_KEY_TEMPLATE, META_KEY_TITLE, TEMPLATE_CLOSE_MARKER, TEMPLATE_OPEN_MARKER,
};
use crate::{utils, AdaptorExt as _, Post};

use pulldown_cmark::{self as md, Parser};
use std::cmp::Ordering;
use std::fs;
use std::io;
use std::ops::Range;
use std::path::Path;

const RULE_CONTENTS: &str = "CONTENTS";
const RULE_CSS: &str = "CSS";
const RULE_TOC: &str = "TOC";
const RULE_LIST: &str = "LIST";
const RULE_META: &str = "META";
const RULE_INCLUDE: &str = "INCLUDE";

#[derive(Clone)]
enum MetaKey {
    Title,
    CreationDate,
    ModifiedDate,
    Category,
    Tags,
    Template,
    Meta(String),
}

#[derive(Clone)]
enum PreprocessorRule {
    Contents,
    Css,
    Toc {
        depth: u8,
    },
    Listing {
        path: String,
        /// (meta key, ascending?)
        sort_by: Option<(MetaKey, bool)>,
    },
    Meta {
        key: String,
    },
    Include {
        path: String,
    },
}

#[derive(Clone)]
struct Replacement {
    range: Range<usize>,
    rule: PreprocessorRule,
}

pub struct HtmlTemplate {
    html: String,
    replacements: Vec<Replacement>,
}

impl MetaKey {
    fn new(value: String) -> Self {
        if value == META_KEY_TITLE {
            Self::Title
        } else if value == META_KEY_CREATION_DATE {
            Self::CreationDate
        } else if value == META_KEY_MODIFIED_DATE {
            Self::ModifiedDate
        } else if value == META_KEY_CATEGORY {
            Self::Category
        } else if value == META_KEY_TAGS {
            Self::Tags
        } else if value == META_KEY_TEMPLATE {
            Self::Template
        } else {
            Self::Meta(value)
        }
    }
}

impl PreprocessorRule {
    fn new(mut string: &str) -> Option<Self> {
        let parsing = &mut string;
        let rule = utils::parse_next_value(parsing)?;
        Some(match rule.as_str() {
            RULE_CONTENTS => PreprocessorRule::Contents,
            RULE_CSS => PreprocessorRule::Css,
            RULE_TOC => {
                let depth = match utils::parse_next_value(parsing) {
                    Some(value) => match value.parse() {
                        Ok(depth) => depth,
                        Err(_) => {
                            eprintln!("note: could not parse depth as a number: {}", string);
                            u8::MAX
                        }
                    },
                    None => u8::MAX,
                };
                PreprocessorRule::Toc { depth }
            }
            RULE_LIST => {
                let path = utils::parse_next_value(parsing)?;

                let mut sort_by = None;
                while let Some(arg) = utils::parse_next_value(parsing) {
                    match arg.as_ref() {
                        "sort" => {
                            match (
                                utils::parse_next_value(parsing),
                                utils::parse_next_value(parsing),
                            ) {
                                (Some(key), Some(order)) if order == "asc" || order == "desc" => {
                                    sort_by = Some((MetaKey::new(key), order == "asc"));
                                }
                                (key, order) => eprintln!(
                                    "note: sort requires key and asc/desc order, but got: {:?}, {:?}",
                                    key, order
                                ),
                            }
                        }
                        _ => eprintln!("note: unrecognized list argument: {}", arg),
                    }
                }

                PreprocessorRule::Listing { path, sort_by }
            }
            RULE_META => {
                let key = utils::parse_next_value(parsing)?;
                PreprocessorRule::Meta { key }
            }
            RULE_INCLUDE => {
                let path = utils::parse_next_value(parsing)?;
                PreprocessorRule::Include { path }
            }
            _ => return None,
        })
    }
}

impl HtmlTemplate {
    pub fn from_file<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let html = fs::read_to_string(path.as_ref())?;
        Ok(Self::new(html, Some(path.as_ref())))
    }

    pub fn from_string(html: String) -> Self {
        Self::new(html, None)
    }

    fn new(html: String, path: Option<&Path>) -> Self {
        let mut replacements = Vec::new();
        let mut offset = 0;
        while let Some(index) = html[offset..].find(TEMPLATE_OPEN_MARKER) {
            let rule_start = offset + index + TEMPLATE_OPEN_MARKER.len();
            let rule_end = match html[rule_start..].find(TEMPLATE_CLOSE_MARKER) {
                Some(i) => rule_start + i,
                None => {
                    eprintln!(
                        "note: html template without close marker after byte offset {}: {:?}",
                        rule_start, path
                    );
                    break;
                }
            };

            let rule = &html[rule_start..rule_end];
            match PreprocessorRule::new(rule) {
                Some(rule) => replacements.push(Replacement {
                    range: (offset + index)..(rule_end + TEMPLATE_CLOSE_MARKER.len()),
                    rule,
                }),
                None => {
                    eprint!(
                        "note: could not understand preprocessor rule {}: {:?}",
                        rule, path
                    );
                }
            }

            offset = rule_end + TEMPLATE_CLOSE_MARKER.len();
        }
        Self { html, replacements }
    }

    pub fn apply(
        &self,
        root: &Path,
        md: &Post,
        files: &[Post],
        css_files: &[String],
    ) -> io::Result<String> {
        let mut html = self.html.clone();
        let mut replacements = self.replacements.clone();
        replacements.sort_by_key(|r| r.range.start);

        for replacement in replacements.into_iter().rev() {
            let value = match replacement.rule {
                PreprocessorRule::Contents => {
                    let mut res = String::new();
                    pulldown_cmark::html::push_html(
                        &mut res,
                        Parser::new(&md.markdown).hyperlink_headings(),
                    );
                    res
                }
                PreprocessorRule::Css => {
                    let mut res = String::new();
                    for css in css_files {
                        let parent = &css[..css.rfind('/').unwrap()];
                        if md.uri.starts_with(parent) {
                            res.push_str(r#"<link rel="stylesheet" type="text/css" href=""#);
                            res.push_str(css);
                            res.push_str("\">");
                        }
                    }
                    res
                }
                PreprocessorRule::Toc { depth: max_depth } => {
                    let mut res = String::new();
                    let mut cur_depth = 0;
                    for (heading, depth) in md.toc.iter() {
                        let depth = *depth;
                        if depth > max_depth {
                            continue;
                        }

                        match cur_depth.cmp(&depth) {
                            Ordering::Less => {
                                while cur_depth != depth {
                                    res.push_str("<ul>");
                                    cur_depth += 1;
                                }
                            }
                            Ordering::Greater => {
                                while cur_depth != depth {
                                    res.push_str("</ul>");
                                    cur_depth -= 1;
                                }
                            }
                            _ => {}
                        }

                        res.push_str("<li>");
                        res.push_str(heading);
                        res.push_str("</li>");
                    }

                    while cur_depth != 0 {
                        res.push_str("</ul>");
                        cur_depth -= 1;
                    }

                    res
                }
                PreprocessorRule::Listing { path, sort_by } => {
                    let path = utils::get_abs_path(root, &md.path, &path);

                    let mut sorted_files;
                    let mut files = files;
                    if let Some((key, asc)) = sort_by {
                        sorted_files = files.to_vec();
                        sorted_files.sort_by(|a, b| {
                            let ordering = match &key {
                                MetaKey::Title => a.title.cmp(&b.title),
                                MetaKey::CreationDate => a.date.cmp(&b.date),
                                MetaKey::ModifiedDate => a.updated.cmp(&b.updated),
                                MetaKey::Category => a.category.cmp(&b.category),
                                MetaKey::Tags => a.tags.cmp(&b.tags),
                                MetaKey::Template => a.template.cmp(&b.template),
                                MetaKey::Meta(key) => a.meta.get(key).cmp(&b.meta.get(key)),
                            };

                            if asc {
                                ordering
                            } else {
                                ordering.reverse()
                            }
                        });
                        files = sorted_files.as_slice();
                    }

                    let mut res = String::new();
                    res.push_str("<ul>");
                    for file in files {
                        if file.path.starts_with(&path) {
                            res.push_str("<li><a href=\"");
                            res.push_str(&utils::get_relative_uri(&md.uri, &file.uri));
                            res.push_str("\">");
                            res.push_str(&file.title);
                            res.push_str("</a></li>");
                        }
                    }
                    res.push_str("</ul>");
                    res
                }
                PreprocessorRule::Meta { key } => {
                    md.meta.get(&key).cloned().unwrap_or_else(String::new)
                }
                PreprocessorRule::Include { path } => {
                    let path = utils::get_abs_path(root, &md.path, &path);

                    match fs::read_to_string(&path) {
                        Ok(s) => {
                            if INCLUDE_RAW_EXTENSIONS.contains(
                                &path
                                    .extension()
                                    .unwrap()
                                    .to_ascii_lowercase()
                                    .to_str()
                                    .unwrap(),
                            ) {
                                s
                            } else {
                                // Add a fourth to the capacity to leave some room for the escapes.
                                // This is merely a best-effort guess to avoid re-allocating.
                                let mut escaped = String::with_capacity(s.len() + s.len() / 4);
                                md::escape::escape_html(&mut escaped, &s).unwrap();
                                escaped
                            }
                        }
                        Err(_) => {
                            eprintln!("note: failed to include {:?}", path);
                            continue;
                        }
                    }
                }
            };

            html.replace_range(replacement.range, &value);
        }

        Ok(html)
    }
}
