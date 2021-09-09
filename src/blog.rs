use crate::{utils, HtmlTemplate, Post, DEFAULT_HTML_TEMPLATE};

use atom_syndication as atom;
use pulldown_cmark::Parser;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::PathBuf;

pub struct Scan {
    /// Root path of the source directory.
    source: PathBuf,
    /// Root path of the destination directory.
    destination: PathBuf,
    /// Directories to create in the destination.
    dirs_to_create: Vec<PathBuf>,
    /// Files to copy to the destination without any special treatment.
    files_to_copy: Vec<PathBuf>,
    /// URIs to the CSS files found.
    css_files: Vec<String>,
    /// HTML templates found.
    html_templates: HashMap<PathBuf, HtmlTemplate>,
    /// HTML template to use when no other file can be used.
    default_template: HtmlTemplate,
    /// Markdown files to parse and generate HTML from.
    md_files: Vec<Post>,
    /// ATOM feeds to fill.
    atom_files: Vec<PathBuf>,
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
    pub fn new(root: PathBuf, dst: PathBuf) -> io::Result<Self> {
        let mut scan = Scan {
            source: root.clone(),
            destination: dst,
            dirs_to_create: Vec::new(),
            files_to_copy: Vec::new(),
            css_files: Vec::new(),
            html_templates: HashMap::new(),
            default_template: HtmlTemplate::new(&root, None, DEFAULT_HTML_TEMPLATE.to_owned()),
            md_files: Vec::new(),
            atom_files: Vec::new(),
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
                        scan.css_files
                            .push(utils::path_to_uri(&root, &entry.path()));
                    }

                    if ext.eq_ignore_ascii_case("atom") {
                        scan.atom_files.push(entry.path());
                    } else if !ext.eq_ignore_ascii_case("md") {
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
        scan.files_to_copy.retain(|path| !templates.contains(path));

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
    pub fn execute(self) -> io::Result<()> {
        if !self.destination.is_dir() {
            fs::create_dir(&self.destination)?;
        }

        let source = self
            .source
            .clone()
            .into_os_string()
            .into_string()
            .expect("bad source path");

        let destination = self
            .destination
            .clone()
            .into_os_string()
            .into_string()
            .expect("bad destination path");

        // Creates all directories that need creating.
        for dir in self.dirs_to_create.iter() {
            // Replace dir's prefix (source) with destination.
            let dir = dir
                .clone()
                .into_os_string()
                .into_string()
                .expect("bad dir path");
            let dir = utils::replace_root(&source, &destination, &dir);
            if !dir.is_dir() {
                fs::create_dir(dir)?;
            }
        }

        // Copies all files that need copying.
        for file in self.files_to_copy.iter() {
            let src = file
                .clone()
                .into_os_string()
                .into_string()
                .expect("bad file path");
            let dst = utils::replace_root(&source, &destination, &src);
            if !dst.is_file() {
                fs::copy(src, dst)?;
            }
        }

        // Generate all feeds.
        for file in self.atom_files.iter() {
            let conf = fs::read_to_string(&file)?;
            let mut conf = conf.lines().map(|l| l.trim()).filter(|l| !l.is_empty());

            let feed_title = conf.next().map(|s| s.to_string()).unwrap_or_else(|| {
                self.source
                    .parent()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned()
            });

            let feed_url = conf
                .next()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "https://example.com".to_owned());

            let uri = utils::path_to_uri(&self.source, &file.parent().unwrap().to_owned());
            let src = file
                .clone()
                .into_os_string()
                .into_string()
                .expect("bad file path");
            let dst = utils::replace_root(&source, &destination, &src);
            let mut entries = Vec::new();
            let mut last_updated = None;

            for md in self.md_files.iter() {
                if md.uri.starts_with(&uri) {
                    if let Some(updated) = last_updated {
                        last_updated = Some(md.updated.max(updated));
                    } else {
                        last_updated = Some(md.updated);
                    }

                    entries.push(atom::Entry {
                        title: md.title.clone().into(),
                        id: {
                            let mut s = feed_url.clone();
                            s.push_str(&md.uri);
                            s
                        },
                        updated: md.updated.and_hms(0, 0, 0).into(),
                        published: Some(md.date.and_hms(0, 0, 0).into()),
                        categories: vec![atom::Category {
                            term: md.category.clone(),
                            ..atom::Category::default()
                        }],
                        content: Some(atom::Content {
                            value: {
                                let mut html = String::new();
                                pulldown_cmark::html::push_html(
                                    &mut html,
                                    Parser::new(&md.markdown),
                                );
                                let mut escaped = String::new();
                                pulldown_cmark::escape::escape_html(&mut escaped, &html).unwrap();
                                Some(escaped)
                            },
                            content_type: Some("html".to_string()),
                            ..atom::Content::default()
                        }),
                        ..atom::Entry::default()
                    });
                }
            }

            fs::write(
                dst,
                atom::Feed {
                    title: feed_title.into(),
                    id: feed_url.clone(),
                    updated: last_updated
                        .map(|d| d.and_hms(0, 0, 0).into())
                        .unwrap_or_else(|| chrono::offset::Local::now().into()),
                    entries,
                    links: vec![atom::Link {
                        rel: "self".into(),
                        href: {
                            let mut s = feed_url;
                            s.push_str(&uri);
                            s
                        },
                        ..atom::Link::default()
                    }],
                    ..atom::Feed::default()
                }
                .to_string(),
            )?;
        }

        // Converts every MD file to HTML and places it in the destination.
        for file in self.md_files.iter() {
            let src = file
                .path
                .clone()
                .with_extension("html")
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

            let html = template.apply(&contents, file, &self.md_files, &self.css_files)?;
            fs::write(dst, html)?;
        }

        Ok(())
    }
}
