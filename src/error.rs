use std::error::Error;
use std::fmt;
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub enum AppError {
    ReadDir {
        source: io::Error,
        path: PathBuf,
    },
    IterDir {
        source: io::Error,
        path: PathBuf,
    },
    WriteDir {
        source: Option<io::Error>,
        path: PathBuf,
        reason: Option<&'static str>,
    },
    DeleteDir {
        source: io::Error,
        path: PathBuf,
    },
    ReadFile {
        source: io::Error,
        path: PathBuf,
    },
    CopyFile {
        source: io::Error,
        src_path: PathBuf,
        dst_path: PathBuf,
    },
    DeleteFile {
        source: Option<io::Error>,
        path: PathBuf,
        reason: &'static str,
    },
    WriteFile {
        source: Option<io::Error>,
        path: PathBuf,
        reason: Option<&'static str>,
    },
    FileMeta {
        source: io::Error,
        path: PathBuf,
    },
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        use AppError::*;

        // TODO I don't know why some arms need the `if let Some`, but it
        //       won't work otherwise. Also tried to `.into()` with no luck.
        match self {
            ReadDir { source, .. } => Some(source),
            IterDir { source, .. } => Some(source),
            WriteDir { source, .. } => {
                if let Some(s) = source {
                    Some(s)
                } else {
                    None
                }
            }
            DeleteDir { source, .. } => Some(source),
            ReadFile { source, .. } => Some(source),
            CopyFile { source, .. } => Some(source),
            DeleteFile { source, .. } => {
                if let Some(s) = source {
                    Some(s)
                } else {
                    None
                }
            }
            WriteFile { source, .. } => {
                if let Some(s) = source {
                    Some(s)
                } else {
                    None
                }
            }
            FileMeta { source, .. } => Some(source),
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use AppError::*;

        match self {
            ReadDir { path, .. } => write!(f, "Failed to read directory: {:?}", path),
            IterDir { path, .. } => write!(f, "Failed to list directory: {:?}", path),
            WriteDir { path, reason, .. } => {
                write!(f, "Failed to create directory")?;
                if let Some(reason) = reason {
                    write!(f, " ({})", reason)?;
                }
                write!(f, ": {:?}", path)
            }
            DeleteDir { path, .. } => write!(f, "Failed to delete directory: {:?}", path),
            ReadFile { path, .. } => write!(f, "Failed to read file: {:?}", path),
            CopyFile {
                src_path, dst_path, ..
            } => write!(f, "Failed to copy file {:?} to {:?}", src_path, dst_path),
            DeleteFile { path, reason, .. } => {
                write!(f, "Failed to delete file ({}): {:?}", reason, path)
            }
            WriteFile { path, reason, .. } => {
                write!(f, "Failed to write file")?;
                if let Some(reason) = reason {
                    write!(f, " ({})", reason)?;
                }
                write!(f, ": {:?}", path)
            }
            FileMeta { path, .. } => write!(f, "Failed to query file metadata: {:?}", path),
        }
    }
}
