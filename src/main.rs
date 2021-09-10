mod blog;
mod feed;
mod post;
mod template;
mod utils;

use post::Post;
use template::HtmlTemplate;

use std::env;
use std::io;

// Program defaults.
pub const SOURCE_PATH: &str = "content";
pub const TARGET_PATH: &str = "dist";

// Source file metadata.
pub const SOURCE_META_KEY: &str = "meta";
pub const DATE_FMT: &str = "%F";
pub const META_KEY_TITLE: &str = "title";
pub const META_KEY_CREATION_DATE: &str = "date";
pub const META_KEY_MODIFIED_DATE: &str = "updated";
pub const META_KEY_CATEGORY: &str = "category";
pub const META_KEY_TAGS: &str = "tags";
pub const META_KEY_TEMPLATE: &str = "template";
pub const META_VALUE_SEPARATOR: &str = "=";
pub const META_TAG_SEPARATOR: &str = ",";

// Template defaults.
pub const DEFAULT_HTML_TEMPLATE: &str = std::include_str!("../template.html");
pub const TEMPLATE_OPEN_MARKER: &str = "<!--P/";
pub const TEMPLATE_CLOSE_MARKER: &str = "/P-->";

// Blog options.
pub const SOURCE_FILE_EXT: &str = "md";
pub const DIST_FILE_EXT: &str = "html";
pub const STYLE_FILE_EXT: &str = "css";
pub const FEED_FILE_EXT: &str = "atom";

// Feed defaults.
pub const FEED_CONTENT_TYPE: &str = "html";
pub const FEED_REL: &str = "self";
pub const FEED_TYPE: &str = "application/atom+xml";

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
