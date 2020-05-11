use std::error::Error;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use pulldown_cmark::{html, Parser};

const DEFAULT_CONTENT_PATH: &str = "content";
const HEADER_FILE_NAME: &str = "header.md";
const FOOTER_FILE_NAME: &str = "footer.md";

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
        dbg!(&child);
        dbg!(&metadata.is_file());

        let file_type = if metadata.is_file() {
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
    let input: String = header.to_string() + body + footer;
    let parser = Parser::new(&input); // TODO options

    let mut output = String::new();
    html::push_html(&mut output, parser);

    output
}

fn get_contents(path: &Path) -> std::io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

fn main() -> Result<(), Box<dyn Error>> {
    let source = get_source_files(DEFAULT_CONTENT_PATH)?;
    dbg!(&source);

    let header = match source.header {
        None => "".into(),
        Some(path) => get_contents(&path)?,
    };
    let footer = match source.footer {
        None => "".into(),
        Some(path) => get_contents(&path)?,
    };

    let first_file_post = source.posts.iter().find(|ps| ps.source_type == File).unwrap();
    let body = get_contents(&first_file_post.path)?;

    dbg!(translate_to_html(&header, &body, &footer));

    todo!()
}

// #[cfg(test)]
// mod tests {
//     #[test]
//     fn it_works() {
//         assert_eq!(2 + 2, 4);
//     }
// }
