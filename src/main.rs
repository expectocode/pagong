mod blog;
mod post;

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

fn main() -> Result<(), Box<dyn Error>> {
    let blog = Blog::from_source_dir(DEFAULT_CONTENT_PATH)?;

    let output_dir = Path::new("dist");
    if !output_dir.exists() {
        eprintln!( "Creating output directory \"{}\"...", output_dir.to_string_lossy());
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

    let actions = blog.generate_actions(output_dir);
    dbg!(&actions);

    todo!()
}
