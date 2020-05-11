use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_CONTENT_PATH: &str = "content";
const HEADER_FILE_NAME: &str = "header.md";
const FOOTER_FILE_NAME: &str = "footer.md";

#[derive(Debug)]
struct BlogSource {
    posts: Vec<PostSource>,
    header: Option<PathBuf>,
    footer: Option<PathBuf>,
}

#[derive(Debug)]
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

fn main() -> Result<(), Box<dyn Error>> {
    let source = get_source_files(DEFAULT_CONTENT_PATH)?;
    dbg!(source);

    todo!()
}

// #[cfg(test)]
// mod tests {
//     #[test]
//     fn it_works() {
//         assert_eq!(2 + 2, 4);
//     }
// }
