use chrono::{NaiveDate, NaiveDateTime};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::io;
use std::ops::Range;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

pub const SOURCE_PATH: &str = "content";
pub const TARGET_PATH: &str = "dist";
pub const DATE_FMT: &str = "%Y-%m-%d";
pub const TEMPLATE_OPEN_MARKER: &str = "<!--P/";
pub const TEMPLATE_CLOSE_MARKER: &str = "/P-->";
pub const DEFAULT_HTML_TEMPLATE: &str = std::include_str!("../template.html");

struct MdFile {
    path: PathBuf,
    title: String,
    date: NaiveDate,
    updated: NaiveDate,
    category: Option<String>,
    tags: Vec<String>,
    template: Option<PathBuf>,
    meta: HashMap<String, String>,
    md_offset: usize,
}

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
    md_files: Vec<MdFile>,
}

/// Parses the next value in the given string. `value` is left at the next value. Parsed value is returned.
fn parse_next_value(string: &mut &str) -> Option<String> {
    let bytes = string.as_bytes();

    let mut offset = 0;
    while offset < bytes.len() {
        if bytes[offset].is_ascii_whitespace() {
            offset += 1;
        } else {
            break;
        }
    }

    if offset == bytes.len() {
        *string = &string[offset..];
        return None;
    }

    let (value, end_offset) = if bytes[offset] == b'"' {
        let mut value = Vec::with_capacity(bytes.len() - offset);
        let mut escape = false;
        let mut index = offset + 1;
        let mut closed = false;
        while index < bytes.len() {
            if escape {
                value.push(bytes[index]);
                escape = false;
            } else {
                if bytes[index] == b'\\' {
                    escape = true;
                } else if bytes[index] == b'"' {
                    closed = true;
                    index += 1;
                    break;
                } else {
                    value.push(bytes[index]);
                }
            }
            index += 1;
        }
        if escape {
            eprintln!(
                "note: reached end of string with escape sequence open: {:?}",
                string
            );
        }
        if !closed {
            eprintln!(
                "note: reached end of string without closing it: {:?}",
                string
            );
        }
        (value, index)
    } else {
        let end_offset = match bytes[offset..].iter().position(|b| b.is_ascii_whitespace()) {
            Some(i) => offset + i,
            None => bytes.len(),
        };
        (bytes[offset..end_offset].to_vec(), end_offset)
    };

    *string = &string[end_offset..];
    String::from_utf8(value).ok()
}

/// Get the absolute path out of value given the root and the path of the file being processed.
fn get_abs_path(root: &PathBuf, path: Option<&PathBuf>, value: &str) -> PathBuf {
    if value.starts_with('/') {
        let mut p = root.clone();
        p.push(&value[1..]);
        p
    } else {
        let mut p = path.unwrap_or(root).clone();
        p.push(value);
        p
    }
}

/// Replace's `path`'s `source` root with `destination`. Panics if `path` does not start with `source`.
///
/// Rust's path (and `OsString`) manipulation is pretty lacking, so the method falls back to `String`.
fn replace_root(source: &String, destination: &String, path: &String) -> PathBuf {
    assert!(path.starts_with(source));
    let rel = &path[source.len() + 1..]; // +1 to skip path separator
    let mut dir = PathBuf::from(&destination);
    dir.push(rel);
    dir
}

fn parse_opt_date(path: &PathBuf, created: bool, string: Option<&String>) -> NaiveDate {
    match string {
        Some(s) => match NaiveDate::parse_from_str(s, DATE_FMT) {
            Ok(d) => return d,
            Err(_) => eprintln!("note: invalid date value: {:?}", s),
        },
        None => {}
    }

    match fs::metadata(&path) {
        Ok(meta) => {
            if created {
                match meta.created() {
                    Ok(date) => {
                        return NaiveDateTime::from_timestamp(
                            date.duration_since(UNIX_EPOCH).unwrap().as_secs() as i64,
                            0,
                        )
                        .date()
                    }
                    Err(_) => eprintln!("note: failed to fetch creation date for file: {:?}", path),
                }
            } else {
                match meta.modified() {
                    Ok(date) => {
                        return NaiveDateTime::from_timestamp(
                            date.duration_since(UNIX_EPOCH).unwrap().as_secs() as i64,
                            0,
                        )
                        .date()
                    }
                    Err(_) => eprintln!(
                        "note: failed to fetch modification date for file: {:?}",
                        path
                    ),
                }
            }
        }
        Err(_) => eprintln!("note: failed to fetch metadata for file: {:?}", path),
    }

    chrono::Local::today().naive_local()
}

impl MdFile {
    pub fn new(root: &PathBuf, path: PathBuf) -> io::Result<Self> {
        let mut meta = HashMap::new();
        let mut md_offset = 0;

        let contents = fs::read_to_string(&path)?;

        if &contents[..4] == "+++\n" {
            if let Some(end_index) = contents.find("\n+++") {
                md_offset = end_index + 4;
                for line in contents[4..end_index].lines() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    let index = match line.find('=') {
                        Some(i) => i,
                        None => {
                            eprintln!("note: metadata line without value: {:?}", line);
                            continue;
                        }
                    };
                    let key = line[..index].trim().to_owned();
                    let value = line[index + 1..].trim().to_owned();
                    meta.insert(key, value);
                }
            } else {
                eprintln!("note: md file with unclosed metadata: {:?}", path);
                md_offset = contents.len();
            }
        } else {
            eprintln!("note: md file without metadata: {:?}", path);
        }

        Ok(MdFile {
            title: match meta.get("title") {
                Some(s) => s.to_owned(),
                None => path.file_name().unwrap().to_str().unwrap().to_owned(),
            },
            date: parse_opt_date(&path, true, meta.get("date")),
            updated: parse_opt_date(&path, false, meta.get("updated")),
            category: meta.get("category").cloned(),
            tags: match meta.get("tags") {
                Some(s) => s.split(',').map(|s| s.trim().to_owned()).collect(),
                None => Vec::new(),
            },
            template: meta
                .get("template")
                .map(|s| get_abs_path(&root, Some(&path), s)),
            path,
            meta,
            md_offset,
        })
    }
}

impl PreprocessorRule {
    fn new(root: &PathBuf, path: Option<&PathBuf>, mut string: &str) -> Option<Self> {
        let parsing = &mut string;
        let rule = parse_next_value(parsing)?;
        Some(match rule.as_str() {
            "CONTENTS" => PreprocessorRule::Contents,
            "CSS" => PreprocessorRule::Css,
            "TOC" => {
                let depth = match parse_next_value(parsing) {
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
                let path = get_abs_path(root, path, &parse_next_value(parsing)?);
                PreprocessorRule::Listing { path }
            }
            "META" => {
                let key = parse_next_value(parsing)?;
                PreprocessorRule::Meta { key }
            }
            "INCLUDE" => {
                let path = get_abs_path(root, path, &parse_next_value(parsing)?);
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

    fn apply(&self, html: &String, md: MdFile) -> io::Result<String> {
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
                PreprocessorRule::Listing { path: _ } => {
                    todo!("list all files")
                }
                PreprocessorRule::Meta { key } => {
                    md.meta.get(&key).cloned().unwrap_or_else(String::new)
                }
                PreprocessorRule::Include { path } => fs::read_to_string(path)?,
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
                        let md = MdFile::new(&scan.source, entry.path())?;
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
            let dir = replace_root(&source, &destination, &dir);
            if !dir.is_dir() {
                fs::create_dir(dir)?;
            }
        }

        // Copies all files that need copying.
        for file in self.files_to_copy {
            let src = file.into_os_string().into_string().expect("bad file path");
            let dst = replace_root(&source, &destination, &src);
            if !dst.is_file() {
                fs::copy(src, dst)?;
            }
        }

        // Converts every MD file to HTML and places it in the destination.
        for file in self.md_files {
            let src = file
                .path
                .clone()
                .into_os_string()
                .into_string()
                .expect("bad md path");
            let dst = replace_root(&source, &destination, &src);

            let (contents, template) = match file.template.clone() {
                Some(tp) => match self.html_templates.get(&tp) {
                    Some(t) => (fs::read_to_string(tp)?, t),
                    None => (DEFAULT_HTML_TEMPLATE.to_owned(), &self.default_template),
                },
                None => (DEFAULT_HTML_TEMPLATE.to_owned(), &self.default_template),
            };

            let html = template.apply(&contents, file)?;
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
            assert_eq!(parse_next_value(&mut string), Some("simple".to_owned()));
        }

        #[test]
        fn quoted() {
            let mut string = "\"quoted\"";
            assert_eq!(parse_next_value(&mut string), Some("quoted".to_owned()));
        }

        #[test]
        fn good_escape() {
            let mut string = "\"good\\\" \\\"escape\"";
            assert_eq!(
                parse_next_value(&mut string),
                Some("good\" \"escape".to_owned())
            );
        }

        #[test]
        fn bad_escape() {
            let mut string = "\"bad\\_escape\"";
            assert_eq!(parse_next_value(&mut string), Some("bad_escape".to_owned()));
        }

        #[test]
        fn unterminated() {
            let mut string = "\"unterminated";
            assert_eq!(
                parse_next_value(&mut string),
                Some("unterminated".to_owned())
            );
        }

        #[test]
        fn multiple() {
            let mut string = " simple \t\"quoted\" \n \"\\\"escapes\\\\\" \n\t \r simple";
            let string = &mut string;
            let mut values = Vec::new();
            while let Some(value) = parse_next_value(string) {
                values.push(value);
            }

            assert_eq!(values, vec!["simple", "quoted", "\"escapes\\", "simple"]);
        }
    }
}
