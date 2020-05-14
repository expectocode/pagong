mod blog;
mod fs_action;
mod post;

mod escape;
mod html;

use std::error::Error;
use std::fs;
use std::io;
use std::path::Path;

pub use blog::Blog;
pub use post::Post;

pub const DEFAULT_CONTENT_PATH: &str = "content";
pub const HEADER_FILE_NAME: &str = "header.md";
pub const FOOTER_FILE_NAME: &str = "footer.md";
pub const FOLDER_POST_NAME: &str = "post.md";
pub const CSS_FILE_NAME: &str = "style.css";
pub const CSS_DIR_NAME: &str = "css";

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

    blog.generate(output_dir)?;

    todo!()
}
