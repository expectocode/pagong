use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use pulldown_cmark::{html, Parser};

const DEFAULT_CONTENT_PATH: &str = "content";
const HEADER_FILE_NAME: &str = "header.md";
const FOOTER_FILE_NAME: &str = "footer.md";
const FOLDER_POST_NAME: &str = "post.md";

#[derive(Debug)]
struct BlogSource {
    posts: Vec<PostSource>,
    header: Option<PathBuf>,
    footer: Option<PathBuf>,
}

#[derive(Debug, PartialEq, Eq)]
enum PostSourceType {
    File,
    Folder,
}
use PostSourceType::*;

#[derive(Debug)]
struct PostSource {
    source_type: PostSourceType,
    path: PathBuf,
}

impl BlogSource {
    fn footer_content(&self) -> io::Result<String> {
        match &self.footer {
            None => Ok("".into()),
            Some(path) => get_content(&path),
        }
    }

    fn header_content(&self) -> io::Result<String> {
        match &self.header {
            None => Ok("".into()),
            Some(path) => get_content(&path),
        }
    }
}

impl PostSource {
    fn content(&self) -> io::Result<String> {
        match self.source_type {
            File => get_content(&self.path),
            Folder => get_content(&self.path.join(FOLDER_POST_NAME)),
        }
    }

    fn file_name(&self) -> Result<&OsStr, &'static str> {
        match self.path.file_name() {
            None => Err("Invalid post file name"),
            Some(name) => Ok(name),
        }
    }
}

fn get_source_files(content_dir: &str) -> Result<BlogSource, Box<dyn Error>> {
    let input = Path::new(content_dir);

    let mut posts = vec![];
    let mut header = None;
    let mut footer = None;

    for child in fs::read_dir(&input)? {
        let child = child?;
        let path = child.path();

        if let Some(name) = path.file_name() {
            if name == HEADER_FILE_NAME {
                header = Some(path);
                continue;
            } else if name == FOOTER_FILE_NAME {
                footer = Some(path);
                continue;
            }
        }

        let metadata = fs::metadata(&path)?;
        let file_type = if fs::metadata(&path)?.is_file() {
            File
        } else if metadata.is_dir() {
            Folder
        } else {
            unreachable!("Followed symlink is not file or directory");
        };

        posts.push(PostSource {
            source_type: file_type,
            path,
        });
    }

    Ok(BlogSource {
        posts,
        header,
        footer,
    })
}

fn translate_to_html(header: &str, body: &str, footer: &str) -> String {
    let input: String = header.to_string() + "\n" + body + "\n" + footer;
    let parser = Parser::new(&input); // TODO options

    let mut output = String::new();
    html::push_html(&mut output, parser);

    output
}

fn get_content(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(content)
}

fn main() -> Result<(), Box<dyn Error>> {
    let source = get_source_files(DEFAULT_CONTENT_PATH)?;

    let output_dir = Path::new("dist");

    let header = source.header_content()?;
    let footer = source.footer_content()?;

    for post in source.posts.iter() {
        let body = post.content()?;

        let mut out_path = output_dir.join(post.file_name()?); // TODO override name with metadata
        out_path.set_extension("html");
        let mut out = fs::File::create(out_path)?;

        let html = translate_to_html(&header, &body, &footer);

        out.write(html.as_bytes())?;
    }

    todo!()
}

// #[cfg(test)]
// mod tests {
//     #[test]
//     fn it_works() {
//         assert_eq!(2 + 2, 4);
//     }
// }
