use std::env;
use std::io;
use std::fs;
use std::path::PathBuf;

pub const SOURCE_PATH: &str = "content";
pub const TARGET_PATH: &str = "dist";

/// Process the non-directory file `src` and put its result in `dst`.
fn process(src: &PathBuf, dst: &mut PathBuf) -> io::Result<()> {
    dst.push(src.file_name().expect("file had no name"));
    // TODO process different files differently
    fs::copy(src, &*dst)?;
    dst.pop();
    Ok(())
}

/// Walks `src`, saving the result to `dst`.
///
/// `src` and `dst` may contain different values if this function exits with `Err`.
fn walk(src: &mut PathBuf, dst: &mut PathBuf) -> io::Result<()> {
    if !dst.is_dir() {
        fs::create_dir(&*dst)?;
    }

    for entry in fs::read_dir(&*src)? {
        let entry = entry?;
        src.push(entry.file_name());

        if src.is_dir() {
            dst.push(entry.file_name());
            walk(src, dst)?;
            dst.pop();
        } else {
            process(src, dst)?;
        }

        src.pop();
    }

    Ok(())
}

fn main() -> io::Result<()> {
    let root = match env::args().nth(1) {
        Some(path) => path.into(),
        None => env::current_dir()?,
    };

    let mut content = root.clone();
    content.push(SOURCE_PATH);

    let mut dist = root;
    dist.push(TARGET_PATH);

    walk(&mut content, &mut dist)?;

    Ok(())
}
