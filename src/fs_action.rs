use crate::AppError;

use std::fs;
use std::path::PathBuf;

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

pub fn execute_fs_actions(actions: &[FsAction]) -> Result<(), AppError> {
    // This code is full of checks which are followed by actions, non-atomically.
    // This means that it's full of TOCTOU race conditions. I don't know how to avoid that.
    for action in actions {
        match action {
            Copy { source, dest } => {
                fs::copy(source, dest).map_err(|e| AppError::CopyFile {
                    source: e,
                    src_path: source.into(),
                    dst_path: dest.into(),
                })?;
            }
            DeleteDir {
                path,
                not_exists_ok,
                recursive,
            } => {
                let should_fail_if_not_exists = !not_exists_ok;
                if !path.exists() {
                    if should_fail_if_not_exists {
                        return Err(AppError::DeleteFile {
                            source: None,
                            path: path.into(),
                            reason: "there is nothing to delete",
                        });
                    }
                    continue;
                }
                if *recursive {
                    fs::remove_dir_all(path).map_err(|e| AppError::DeleteDir {
                        source: e,
                        path: path.into(),
                    })?;
                } else {
                    // Requires that the directory is empty
                    fs::remove_dir(path).map_err(|e| AppError::DeleteDir {
                        source: e,
                        path: path.into(),
                    })?;
                }
            }
            CreateDir { path, exists_ok } => {
                if *exists_ok && path.exists() {
                    if !path.is_dir() {
                        return Err(AppError::WriteDir {
                            source: None,
                            path: path.into(),
                            reason: Some("a file already exists"),
                        });
                    }
                    return Ok(());
                }
                fs::create_dir(path).map_err(|e| AppError::WriteDir {
                    source: Some(e),
                    path: path.into(),
                    reason: None,
                })?;
            }
            WriteFile { path, content } => {
                if path.exists() && !path.is_file() {
                    return Err(AppError::WriteFile {
                        source: None,
                        path: path.into(),
                        reason: Some("a directory already exists"),
                    });
                }

                // fs::write handles creation and truncation for us.
                fs::write(path, content).map_err(|e| AppError::WriteFile {
                    source: Some(e),
                    path: path.into(),
                    reason: None,
                })?;
            }
        }
    }

    Ok(())
}
