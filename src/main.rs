mod blog;
mod config;
mod feed;
mod post;
mod template;
mod utils;

use post::Post;
use template::HtmlTemplate;

use std::io;

fn main() -> io::Result<()> {
    let config = config::parse_cli_args()?;

    let mut content = config.root.clone();
    content.push(config::SOURCE_PATH);

    let mut dist = config.root.clone();
    dist.push(config::TARGET_PATH);

    let scan = blog::scan_dir(&config, content)?;
    blog::generate_from_scan(&config, scan, dist)?;

    Ok(())
}
