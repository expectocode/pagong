use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};

#[derive(Debug)]
pub enum FsAction {
    Copy {
        source: PathBuf,
        dest: PathBuf,
    },
    DeleteDir {
        path: PathBuf,
        not_exists_ok: bool,
        recursive: bool,
    },
    CreateDir {
        path: PathBuf,
        exists_ok: bool,
    },

    /// Creates file if it does not exist, overwrites if it does exist.
    WriteFile {
        path: PathBuf,
        content: String,
    },
}
use FsAction::*;

pub fn execute_fs_actions(actions: &[FsAction]) -> Result<()> {
    // This code is full of checks which are followed by actions, non-atomically.
    // This means that it's full of TOCTOU race conditions. I don't know how to avoid that.
    for action in actions {
        match action {
            Copy { source, dest } => {
                fs::copy(source, dest)
                    .context(format!("Could not copy '{:?}' to '{:?}'", source, dest))?;
            }
            DeleteDir {
                path,
                not_exists_ok,
                recursive,
            } => {
                let should_fail_if_not_exists = !not_exists_ok;
                if !path.exists() {
                    if should_fail_if_not_exists {
                        return Err(anyhow!(
                            "Path '{:?}' could not be deleted because it does not exist",
                            path
                        ));
                    }
                    continue;
                }
                if *recursive {
                    fs::remove_dir_all(path).context(format!(
                        "Could not recursively delete directory '{:?}'",
                        path
                    ))?;
                } else {
                    // Requires that the directory is empty
                    fs::remove_dir(path)
                        .context(format!("Could not delete directory '{:?}'", path))?;
                }
            }
            CreateDir { path, exists_ok } => {
                if *exists_ok && path.exists() {
                    if !path.is_dir() {
                        return Err(anyhow!(
                            "Could not create directory '{:?}': a file already exists",
                            path
                        ));
                    }
                    return Ok(());
                }
                fs::create_dir(path).context(format!("Could not create directory '{:?}'", path))?;
            }
            WriteFile { path, content } => {
                if path.exists() && !path.is_file() {
                    return Err(anyhow!(
                        "Could not write file '{:?}': a directory already exists"
                    ));
                }

                // fs::write handles creation and truncation for us.
                fs::write(path, content).context(format!("Could not write file '{:?}'", path))?;
            }
        }
    }

    Ok(())
}
