use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct Blog {
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

fn main() -> Result<(), Box<dyn Error>> {
    let input = Path::new("content");

    let mut posts = vec![];
    let mut header = None;
    let mut footer = None;

    for child in fs::read_dir(&input)? {
        let child = child?;
        let path = child.path();

        if path.file_name() == Some(OsStr::new("header.md")) {
            header = Some(path);
            continue;
        } else if path.file_name() == Some(OsStr::new("footer.md")) {
            footer = Some(path);
            continue;
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

    let blog = Blog {
        posts,
        header,
        footer,
    };

    dbg!(blog);

    todo!()
}

// #[cfg(test)]
// mod tests {
//     #[test]
//     fn it_works() {
//         assert_eq!(2 + 2, 4);
//     }
// }
