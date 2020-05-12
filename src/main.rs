mod blog;
mod post;

use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

use pulldown_cmark::{html, Parser};

pub use blog::Blog;
pub use post::Post;

pub const DEFAULT_CONTENT_PATH: &str = "content";
pub const HEADER_FILE_NAME: &str = "header.md";
pub const FOOTER_FILE_NAME: &str = "footer.md";
pub const FOLDER_POST_NAME: &str = "post.md";

fn translate_to_html(header: &str, body: &str, footer: &str) -> String {
    let input: String = header.to_string() + "\n" + body + "\n" + footer;
    let parser = Parser::new(&input); // TODO options

    let mut output = String::new();
    html::push_html(&mut output, parser);

    output
}

fn main() -> Result<(), Box<dyn Error>> {
    let blog = Blog::from_source_dir(DEFAULT_CONTENT_PATH)?;

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

    let Blog {
        posts,
        header,
        footer,
    } = blog;

    for post in posts.into_iter() {
        // TODO override name with metadata
        let post_file_stem = post.source.file_stem().expect("Post must have filename");
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
