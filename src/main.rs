mod blog;
mod error;
mod escape;
mod fs_action;
mod html;
mod post;

use std::fs;
use std::path::Path;

pub use blog::Blog;
pub use error::AppError;
pub use post::Post;

pub const DEFAULT_CONTENT_PATH: &str = "content";
pub const HEADER_FILE_NAME: &str = "header.md";
pub const FOOTER_FILE_NAME: &str = "footer.md";
pub const FOLDER_POST_NAME: &str = "post.md";
pub const CSS_FILE_NAME: &str = "style.css";
pub const CSS_DIR_NAME: &str = "css";

fn run() -> Result<(), AppError> {
    let blog = Blog::from_source_dir(DEFAULT_CONTENT_PATH)?;

    let output_dir = Path::new("dist");
    if !output_dir.exists() {
        eprintln!(
            "Creating output directory \"{}\"...",
            output_dir.to_string_lossy()
        );
        fs::create_dir(output_dir).map_err(|e| AppError::WriteDir {
            source: Some(e),
            path: output_dir.into(),
            reason: None,
        })?;
    }
    if !output_dir.is_dir() {
        return Err(AppError::WriteDir {
            source: None,
            path: output_dir.into(),
            reason: Some("already taken by a file"),
        });
    }

    blog.generate(output_dir)?;

    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("FATAL: {}", err);
        std::process::exit(-1);
    }
}
