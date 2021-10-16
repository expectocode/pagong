use crate::config::{TEMPLATE_CLOSE_MARKER, TEMPLATE_OPEN_MARKER};
use crate::{utils, Post};

use pulldown_cmark::Parser;
use std::cmp::Ordering;
use std::fs;
use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};

const RULE_CONTENTS: &str = "CONTENTS";
const RULE_CSS: &str = "CSS";
const RULE_TOC: &str = "TOC";
const RULE_LIST: &str = "LIST";
const RULE_META: &str = "META";
const RULE_INCLUDE: &str = "INCLUDE";

#[derive(Clone)]
enum PreprocessorRule {
    Contents,
    Css,
    Toc { depth: u8 },
    Listing { path: PathBuf },
    Meta { key: String },
    Include { path: PathBuf },
}

#[derive(Clone)]
struct Replacement {
    range: Range<usize>,
    rule: PreprocessorRule,
}

pub struct HtmlTemplate {
    replacements: Vec<Replacement>,
}

impl PreprocessorRule {
    fn new(root: &Path, path: Option<&Path>, mut string: &str) -> Option<Self> {
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
                let path = utils::get_abs_path(root, path, &utils::parse_next_value(parsing)?);
                PreprocessorRule::Listing { path }
            }
            RULE_META => {
                let key = utils::parse_next_value(parsing)?;
                PreprocessorRule::Meta { key }
            }
            RULE_INCLUDE => {
                let path = utils::get_abs_path(root, path, &utils::parse_next_value(parsing)?);
                PreprocessorRule::Include { path }
            }
            _ => return None,
        })
    }
}

impl HtmlTemplate {
    pub fn load(root: &Path, path: &Path) -> io::Result<Self> {
        let contents = fs::read_to_string(&path)?;
        Ok(Self::new(root, Some(path), contents))
    }

    pub fn new(root: &Path, path: Option<&Path>, contents: String) -> Self {
        let mut replacements = Vec::new();
        let mut offset = 0;
        while let Some(index) = contents[offset..].find(TEMPLATE_OPEN_MARKER) {
            let rule_start = offset + index + TEMPLATE_OPEN_MARKER.len();
            let rule_end = match contents[rule_start..].find(TEMPLATE_CLOSE_MARKER) {
                Some(i) => rule_start + i,
                None => {
                    eprintln!(
                        "note: html template without close marker after byte offset {}: {:?}",
                        rule_start, path
                    );
                    break;
                }
            };

            let rule = &contents[rule_start..rule_end];
            match PreprocessorRule::new(root, path, rule) {
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
        Self { replacements }
    }

    pub fn apply(
        &self,
        mut html: String,
        md: &Post,
        files: &[Post],
        css_files: &[String],
    ) -> io::Result<String> {
        let mut replacements = self.replacements.clone();
        replacements.sort_by_key(|r| r.range.start);

        for replacement in replacements.into_iter().rev() {
            let value = match replacement.rule {
                PreprocessorRule::Contents => {
                    let mut res = String::new();
                    pulldown_cmark::html::push_html(&mut res, Parser::new(&md.markdown));
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
                PreprocessorRule::Listing { path } => {
                    // TODO would like ordering and different formattings
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
                // TODO escape non-html
                PreprocessorRule::Include { path } => match fs::read_to_string(&path) {
                    Ok(s) => s,
                    Err(_) => {
                        eprintln!("note: failed to include {:?}", path);
                        continue;
                    }
                },
            };

            html.replace_range(replacement.range, &value);
        }

        Ok(html)
    }
}
