use std::path::PathBuf;
use std::{fs, io};

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

pub fn execute_fs_actions(actions: &[FsAction]) -> io::Result<()> {
    // This code is full of checks which are followed by actions, non-atomically.
    // This means that it's full of TOCTOU race conditions. I don't know how to avoid that.
    for action in actions {
        match action {
            Copy { source, dest } => {
                fs::copy(source, dest)?;
            }
            DeleteDir {
                path,
                not_exists_ok,
                recursive,
            } => {
                let should_fail_if_not_exists = !not_exists_ok;
                if !path.exists() {
                    if should_fail_if_not_exists {
                        return Err(io::Error::new(
                            io::ErrorKind::NotFound,
                            format!("There is nothing to delete at {}", path.to_string_lossy()),
                        ));
                    }
                    continue;
                }
                if *recursive {
                    fs::remove_dir_all(path)?;
                } else {
                    // Requires that the directory is empty
                    fs::remove_dir(path)?;
                }
            }
            CreateDir { path, exists_ok } => {
                if *exists_ok && path.exists() {
                    if !path.is_dir() {
                        return Err(io::Error::new(
                            io::ErrorKind::AlreadyExists,
                            format!(
                                "There is already a file (not a directory) at {}",
                                path.to_string_lossy()
                            ),
                        ));
                    }
                    return Ok(());
                }
                fs::create_dir(path)?;
            }
            WriteFile { path, content } => {
                if path.exists() && !path.is_file() {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        format!(
                            "There is already a directory (not a file) at {}",
                            path.to_string_lossy()
                        ),
                    ));
                }

                // fs::write handles creation and truncation for us.
                fs::write(path, content)?;
            }
        }
    }

    Ok(())
}
