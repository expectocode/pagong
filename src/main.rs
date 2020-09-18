mod blog;
mod escape;
mod fs_action;
mod html;
mod post;

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};

pub use blog::Blog;
pub use post::Post;

pub const DEFAULT_CONTENT_PATH: &str = "content";
pub const HEADER_FILE_NAME: &str = "header.md";
pub const FOOTER_FILE_NAME: &str = "footer.md";
pub const FOLDER_POST_NAME: &str = "post.md";
pub const CSS_FILE_NAME: &str = "style.css";
pub const CSS_DIR_NAME: &str = "css";

fn main() -> Result<()> {
    let blog = Blog::from_source_dir(DEFAULT_CONTENT_PATH)?;

    let output_dir = Path::new("dist");
    if !output_dir.exists() {
        eprintln!(
            "Creating output directory \"{}\"...",
            output_dir.to_string_lossy()
        );
        fs::create_dir(output_dir).context("Could not create output directory")?;
    }

    // TODO consider using FsAction here for DRY
    if !output_dir.is_dir() {
        return Err(anyhow!(
            "Could not create output directory: path '{:?}' already exists",
            output_dir
        ));
    }

    blog.generate(output_dir)?;

    Ok(())
}
