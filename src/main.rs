mod post;
mod utils;

use post::Post;

use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::io;
use std::ops::Range;
use std::path::PathBuf;

pub const SOURCE_PATH: &str = "content";
pub const TARGET_PATH: &str = "dist";
pub const DATE_FMT: &str = "%F";
pub const TEMPLATE_OPEN_MARKER: &str = "<!--P/";
pub const TEMPLATE_CLOSE_MARKER: &str = "/P-->";
pub const DEFAULT_HTML_TEMPLATE: &str = std::include_str!("../template.html");

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

struct HtmlTemplate {
    replacements: Vec<Replacement>,
}

struct Scan {
    /// Root path of the source directory.
    source: PathBuf,
    /// Root path of the destination directory.
    destination: PathBuf,
    /// Directories to create in the destination.
    dirs_to_create: Vec<PathBuf>,
    /// Files to copy to the destination without any special treatment.
    files_to_copy: Vec<PathBuf>,
    /// CSS files found.
    css_files: Vec<PathBuf>,
    /// HTML templates found.
    html_templates: HashMap<PathBuf, HtmlTemplate>,
    /// HTML template to use when no other file can be used.
    default_template: HtmlTemplate,
    /// Markdown files to parse and generate HTML from.
    md_files: Vec<Post>,
}

impl PreprocessorRule {
    fn new(root: &PathBuf, path: Option<&PathBuf>, mut string: &str) -> Option<Self> {
        let parsing = &mut string;
        let rule = utils::parse_next_value(parsing)?;
        Some(match rule.as_str() {
            "CONTENTS" => PreprocessorRule::Contents,
            "CSS" => PreprocessorRule::Css,
            "TOC" => {
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
            "LIST" => {
                let path = utils::get_abs_path(root, path, &utils::parse_next_value(parsing)?);
                PreprocessorRule::Listing { path }
            }
            "META" => {
                let key = utils::parse_next_value(parsing)?;
                PreprocessorRule::Meta { key }
            }
            "INCLUDE" => {
                let path = utils::get_abs_path(root, path, &utils::parse_next_value(parsing)?);
                PreprocessorRule::Include { path }
            }
            _ => return None,
        })
    }
}

impl HtmlTemplate {
    fn load(root: &PathBuf, path: &PathBuf) -> io::Result<Self> {
        let contents = fs::read_to_string(&path)?;
        Ok(Self::new(root, Some(path), contents))
    }

    fn new(root: &PathBuf, path: Option<&PathBuf>, contents: String) -> Self {
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

    fn apply(&self, html: &String, md: &Post, files: &Vec<Post>) -> io::Result<String> {
        let mut result = html.clone();
        let mut replacements = self.replacements.clone();
        replacements.sort_by_key(|r| r.range.start);

        for replacement in replacements.into_iter().rev() {
            let value = match replacement.rule {
                PreprocessorRule::Contents => fs::read_to_string(&md.path)?,
                PreprocessorRule::Css => {
                    todo!("determine all css that apply")
                }
                PreprocessorRule::Toc { depth: _ } => {
                    todo!("determine toc from md")
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

            result.replace_range(replacement.range, &value);
        }

        Ok(result)
    }
}

impl Scan {
    /// Creates a new scan on two stages. The first stage:
    ///
    /// * Detects all directories that need to be created.
    /// * Detects all CSS files.
    /// * Marks every file as needing a copy except for MD files.
    /// * Parses all MD files.
    ///
    /// The second stage:
    ///
    /// * Removes the HTML templates from the files that need copying.
    fn new(root: PathBuf, dst: PathBuf) -> io::Result<Self> {
        let mut scan = Scan {
            source: root.clone(),
            destination: dst,
            dirs_to_create: Vec::new(),
            files_to_copy: Vec::new(),
            css_files: Vec::new(),
            html_templates: HashMap::new(),
            default_template: HtmlTemplate::new(&root, None, DEFAULT_HTML_TEMPLATE.to_owned()),
            md_files: Vec::new(),
        };
        let mut templates = HashSet::new();

        let mut pending = vec![scan.source.clone()];
        while let Some(src) = pending.pop() {
            for entry in fs::read_dir(&src)? {
                let entry = entry?;

                if entry.file_type()?.is_dir() {
                    pending.push(entry.path());
                    // Detects all directories that need to be created.
                    scan.dirs_to_create.push(entry.path());
                } else {
                    let filename = entry.file_name();
                    let filename = filename.to_str().expect("bad filename");
                    let ext_idx = filename
                        .rfind('.')
                        .map(|i| i + 1)
                        .unwrap_or_else(|| filename.len());
                    let ext = &filename[ext_idx..];

                    if ext.eq_ignore_ascii_case("css") {
                        // Detects all CSS files.
                        scan.css_files.push(entry.path());
                    }
                    if !ext.eq_ignore_ascii_case("md") {
                        // Marks every file as needing a copy except for MD files.
                        scan.files_to_copy.push(entry.path());
                    } else {
                        // Parses all MD files.
                        let md = Post::new(&scan.source, entry.path())?;
                        if let Some(template) = md.template.as_ref() {
                            templates.insert(template.clone());
                        }
                        scan.md_files.push(md);
                    }
                }
            }
        }

        // Removes the HTML templates from the files that need copying.
        scan.files_to_copy.retain(|path| templates.contains(path));

        // Parse templates.
        scan.html_templates.extend(templates.into_iter().filter_map(
            |path| match HtmlTemplate::load(&root, &path) {
                Ok(template) => Some((path, template)),
                Err(_) => {
                    eprintln!("note: failed to parse html template: {:?}", path);
                    None
                }
            },
        ));

        Ok(scan)
    }

    /// Executes the scan:
    ///
    /// * Creates all directories that need creating.
    /// * Copies all files that need copying.
    /// * Converts every MD file to HTML and places it in the destination.
    fn execute(self) -> io::Result<()> {
        let source = self
            .source
            .into_os_string()
            .into_string()
            .expect("bad source path");

        let destination = self
            .destination
            .into_os_string()
            .into_string()
            .expect("bad destination path");

        // Creates all directories that need creating.
        for dir in self.dirs_to_create {
            // Replace dir's prefix (source) with destination.
            let dir = dir.into_os_string().into_string().expect("bad dir path");
            let dir = utils::replace_root(&source, &destination, &dir);
            if !dir.is_dir() {
                fs::create_dir(dir)?;
            }
        }

        // Copies all files that need copying.
        for file in self.files_to_copy {
            let src = file.into_os_string().into_string().expect("bad file path");
            let dst = utils::replace_root(&source, &destination, &src);
            if !dst.is_file() {
                fs::copy(src, dst)?;
            }
        }

        // Converts every MD file to HTML and places it in the destination.
        for file in self.md_files.iter() {
            let src = file
                .path
                .clone()
                .into_os_string()
                .into_string()
                .expect("bad md path");
            let dst = utils::replace_root(&source, &destination, &src);

            let (contents, template) = match file.template.clone() {
                Some(tp) => match self.html_templates.get(&tp) {
                    Some(t) => (fs::read_to_string(tp)?, t),
                    None => (DEFAULT_HTML_TEMPLATE.to_owned(), &self.default_template),
                },
                None => (DEFAULT_HTML_TEMPLATE.to_owned(), &self.default_template),
            };

            let html = template.apply(&contents, file, &self.md_files)?;
            fs::write(dst, html)?;
        }

        Ok(())
    }
}

fn main() -> io::Result<()> {
    let root = match env::args().nth(1) {
        Some(path) => path.into(),
        None => env::current_dir()?,
    };

    let mut content = root.clone();
    content.push(SOURCE_PATH);

    let mut dist = root;
    dist.push(TARGET_PATH);

    Scan::new(content, dist)?.execute()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    mod parse_value {
        use super::*;

        #[test]
        fn simple() {
            let mut string = "simple";
            assert_eq!(
                utils::parse_next_value(&mut string),
                Some("simple".to_owned())
            );
        }

        #[test]
        fn quoted() {
            let mut string = "\"quoted\"";
            assert_eq!(
                utils::parse_next_value(&mut string),
                Some("quoted".to_owned())
            );
        }

        #[test]
        fn good_escape() {
            let mut string = "\"good\\\" \\\"escape\"";
            assert_eq!(
                utils::parse_next_value(&mut string),
                Some("good\" \"escape".to_owned())
            );
        }

        #[test]
        fn bad_escape() {
            let mut string = "\"bad\\_escape\"";
            assert_eq!(
                utils::parse_next_value(&mut string),
                Some("bad_escape".to_owned())
            );
        }

        #[test]
        fn unterminated() {
            let mut string = "\"unterminated";
            assert_eq!(
                utils::parse_next_value(&mut string),
                Some("unterminated".to_owned())
            );
        }

        #[test]
        fn multiple() {
            let mut string = " simple \t\"quoted\" \n \"\\\"escapes\\\\\" \n\t \r simple";
            let string = &mut string;
            let mut values = Vec::new();
            while let Some(value) = utils::parse_next_value(string) {
                values.push(value);
            }

            assert_eq!(values, vec!["simple", "quoted", "\"escapes\\", "simple"]);
        }
    }
}
