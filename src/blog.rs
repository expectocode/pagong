use crate::config::{Config, SOURCE_FILE_EXT, STYLE_FILE_EXT};
use crate::{feed, utils, HtmlTemplate, Post};

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::PathBuf;

pub struct Scan {
    /// Root path of the source directory.
    root: PathBuf,
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
    atom_files: Vec<feed::Meta>,
}

/// Scan a directory containing a blog made up of markdown files, templates and assets.
pub fn scan_dir(config: &Config, root: PathBuf) -> io::Result<Scan> {
    let mut dirs_to_create = Vec::new();
    let mut css_files = Vec::new();
    let mut atom_files = Vec::new();
    let mut files_to_copy = Vec::new();
    let mut md_files = Vec::new();
    let mut templates = HashSet::new();

    let mut pending = vec![root.clone()];
    while let Some(src) = pending.pop() {
        for entry in fs::read_dir(src)? {
            let entry = entry?;

            if entry.file_type()?.is_dir() {
                pending.push(entry.path());
                // Detects all directories that need to be created.
                dirs_to_create.push(entry.path());
            } else {
                let filename = entry.file_name();
                let filename = filename.to_str().expect("bad filename");
                let ext_idx = filename
                    .rfind('.')
                    .map(|i| i + 1)
                    .unwrap_or_else(|| filename.len());
                let ext = &filename[ext_idx..];

                if ext.eq_ignore_ascii_case(STYLE_FILE_EXT) {
                    // Detects all CSS files.
                    css_files.push(utils::path_to_uri(&root, &entry.path()));
                }

                if ext.eq_ignore_ascii_case(&config.feed_ext) {
                    match feed::load_atom_feed(&entry.path()) {
                        Ok(atom) => atom_files.push(atom),
                        Err(e) => {
                            eprintln!("note: failed to load atom feed: {}: {:?}", e, entry.path());
                            files_to_copy.push(entry.path());
                        }
                    }
                } else if !ext.eq_ignore_ascii_case(SOURCE_FILE_EXT) {
                    // Marks every file as needing a copy except for MD files.
                    files_to_copy.push(entry.path());
                } else {
                    // Parses all MD files.
                    let md = Post::new(config, &root, entry.path())?;
                    if let Some(template) = md.template.as_ref() {
                        templates.insert(template.clone());
                    }
                    md_files.push(md);
                }
            }
        }
    }

    // Removes the HTML templates from the files that need copying.
    files_to_copy.retain(|path| !templates.contains(path));

    // Parse templates.
    let default_template = HtmlTemplate::new(&root, None, config.template.clone());
    let html_templates = templates
        .into_iter()
        .filter_map(|path| match HtmlTemplate::load(&root, &path) {
            Ok(template) => Some((path, template)),
            Err(_) => {
                eprintln!("note: failed to parse html template: {:?}", path);
                None
            }
        })
        .collect();

    Ok(Scan {
        root,
        dirs_to_create,
        files_to_copy,
        css_files,
        html_templates,
        default_template,
        md_files,
        atom_files,
    })
}

/// Generate a blog from a previous `Scan`, turning all source files into HTML.
pub fn generate_from_scan(config: &Config, scan: Scan, destination: PathBuf) -> io::Result<()> {
    if !destination.is_dir() {
        fs::create_dir(&destination)?;
    }

    let source = scan
        .root
        .clone()
        .into_os_string()
        .into_string()
        .expect("bad source path");

    let destination = destination
        .into_os_string()
        .into_string()
        .expect("bad destination path");

    // Creates all directories that need creating.
    for dir in scan.dirs_to_create.iter() {
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
    for file in scan.files_to_copy.iter() {
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
    for atom in scan.atom_files {
        let src = atom
            .path
            .clone()
            .into_os_string()
            .into_string()
            .expect("bad file path");

        let dst = utils::replace_root(&source, &destination, &src);
        fs::write(dst, feed::fill_atom_feed(atom, &scan.md_files))?;
    }

    // Converts every MD file to HTML and places it in the destination.
    for file in scan.md_files.iter() {
        let src = file
            .path
            .clone()
            .with_extension(&config.dist_ext)
            .into_os_string()
            .into_string()
            .expect("bad md path");
        let dst = utils::replace_root(&source, &destination, &src);

        let (contents, template) = match file.template.clone() {
            Some(tp) => match scan.html_templates.get(&tp) {
                Some(t) => (fs::read_to_string(tp)?, t),
                None => (config.template.clone(), &scan.default_template),
            },
            None => (config.template.clone(), &scan.default_template),
        };

        let html = template.apply(contents, file, &scan.md_files, &scan.css_files)?;
        fs::write(dst, html)?;
    }

    Ok(())
}
