use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use chrono::offset::Local;
use chrono::DateTime;
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

#[derive(Debug)]
struct Post {
    source: PostSource,
    markdown: String,
    title: String,
    modified: DateTime<Local>,
    created: DateTime<Local>,
    assets: Vec<PathBuf>,
}

impl Post {
    fn from_post_source(source: PostSource) -> Result<Post, Box<dyn Error>> {
        let post_path = match source.source_type {
            File => source.path.clone(),
            Folder => source.path.join(FOLDER_POST_NAME),
        };

        let content = get_content(&post_path)?;

        let post_metadata = post_path.metadata()?;
        let created = post_metadata.created()?.into();
        let modified = post_metadata.modified()?.into();

        let mut assets = vec![];
        if let Folder = source.source_type {
            for child in fs::read_dir(&source.path)? {
                let child = child?;
                if child.path().extension() != Some(&OsStr::new("md")) {
                    // don't add .md files as assets
                    assets.push(child.path());
                }
            }
        }

        Ok(Post {
            source,
            markdown: content,
            title: "Title".into(), // TODO
            modified,
            created,
            assets,
        })
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
    if !output_dir.exists() {
        eprintln!(
            "Creating output directory \"{}\"...",
            output_dir.to_string_lossy()
        );
        fs::create_dir(output_dir)?;
    }
    if !output_dir.is_dir() {
        return Err(Box::new(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "Output directory name {} is already taken by a file. Please move or remove it",
                output_dir.to_string_lossy()
            ),
        )));
    }

    let header = source.header_content()?;
    let footer = source.footer_content()?;

    for post_source in source.posts.into_iter() {
        let post = Post::from_post_source(post_source)?;

        // TODO override name with metadata
        let post_file_stem = post
            .source
            .path
            .file_stem()
            .expect("Post must have filename");
        let post_dir = output_dir.join(post_file_stem);
        if post_dir.exists() {
            fs::remove_dir_all(&post_dir)?;
        }
        fs::create_dir(&post_dir)?;

        let post_path = post_dir.join("index.html");
        let mut out = fs::File::create(post_path)?;

        let html = translate_to_html(&header, &post.markdown, &footer);

        for asset in post.assets {
            let asset_name = asset.file_name().expect("Asset must have file name");
            let dest_path = post_dir.join(asset_name);
            fs::copy(asset, dest_path).expect("File copy failed");
        }

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
