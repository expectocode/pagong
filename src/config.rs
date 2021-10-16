use crate::HtmlTemplate;

use clap::{App, Arg};
use std::env;
use std::io;
use std::path::PathBuf;

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

pub struct Config {
    pub root: PathBuf,
    pub template: HtmlTemplate,
    pub dist_ext: String,
    pub feed_ext: String,
}

pub fn parse_cli_args() -> io::Result<Config> {
    let config = App::new("pagong")
        .version("0.1.1")
        .author("expectocode <expectocode@gmail.com>, Lonami Exo <totufals@hotmail.com>")
        .about("A static site generator for slow connections")
        .arg(Arg::with_name("root")
            .value_name("SOURCE ROOT")
            .help("Sets the root directory where the program should run [default: current directory]"))
        .arg(Arg::with_name("template")
            .value_name("TEMPLATE")
            .short("t")
            .long("default-template")
            .help("Sets the default HTML template for the source Markdown files [default: basic embedded template]"))
        .arg(Arg::with_name("dist_ext")
            .value_name("EXT")
            .short("e")
            .long("generated-extension")
            .help("Sets the file extension for the converted Markdown files")
            .default_value("html"))
        .arg(Arg::with_name("feed_ext")
            .value_name("EXT")
            .short("a")
            .long("feed-extension")
            .help("Sets the file extension used for the Atom feed files")
            .default_value("atom"))
        .get_matches();

    let root = match config.value_of("root") {
        Some(path) => path.into(),
        None => env::current_dir()?,
    };

    let template = match config.value_of("template") {
        Some(path) => HtmlTemplate::from_file(path)?,
        None => HtmlTemplate::from_string(DEFAULT_HTML_TEMPLATE.to_string()),
    };

    let dist_ext = match config.value_of("dist_ext") {
        Some(ext) => ext.to_string(),
        None => DIST_FILE_EXT.to_string(),
    };

    let feed_ext = match config.value_of("feed_ext") {
        Some(ext) => ext.to_string(),
        None => FEED_FILE_EXT.to_string(),
    };

    Ok(Config {
        root,
        template,
        dist_ext,
        feed_ext,
    })
}
