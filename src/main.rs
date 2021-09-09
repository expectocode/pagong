mod blog;
mod post;
mod template;
mod utils;

use post::Post;
use template::HtmlTemplate;

use std::env;
use std::io;

pub const SOURCE_PATH: &str = "content";
pub const TARGET_PATH: &str = "dist";
pub const DATE_FMT: &str = "%F";
pub const TEMPLATE_OPEN_MARKER: &str = "<!--P/";
pub const TEMPLATE_CLOSE_MARKER: &str = "/P-->";
pub const DEFAULT_HTML_TEMPLATE: &str = std::include_str!("../template.html");

fn main() -> io::Result<()> {
    let root = match env::args().nth(1) {
        Some(path) => path.into(),
        None => env::current_dir()?,
    };

    let mut content = root.clone();
    content.push(SOURCE_PATH);

    let mut dist = root;
    dist.push(TARGET_PATH);

    let scan = blog::scan_dir(content)?;
    blog::generate_from_scan(scan, dist)?;

    Ok(())
}
