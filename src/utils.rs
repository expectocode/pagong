use chrono::{Date, Local, NaiveDate, NaiveDateTime, TimeZone as _};
use std::fs;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

/// Parses the next value in the given string. `value` is left at the next value. Parsed value is returned.
pub fn parse_next_value(string: &mut &str) -> Option<String> {
    let bytes = string.as_bytes();

    let mut offset = 0;
    while offset < bytes.len() {
        if bytes[offset].is_ascii_whitespace() {
            offset += 1;
        } else {
            break;
        }
    }

    if offset == bytes.len() {
        *string = &string[offset..];
        return None;
    }

    let (value, end_offset) = if bytes[offset] == b'"' {
        let mut value = Vec::with_capacity(bytes.len() - offset);
        let mut escape = false;
        let mut index = offset + 1;
        let mut closed = false;
        while index < bytes.len() {
            if escape {
                value.push(bytes[index]);
                escape = false;
            } else {
                if bytes[index] == b'\\' {
                    escape = true;
                } else if bytes[index] == b'"' {
                    closed = true;
                    index += 1;
                    break;
                } else {
                    value.push(bytes[index]);
                }
            }
            index += 1;
        }
        if escape {
            eprintln!(
                "note: reached end of string with escape sequence open: {:?}",
                string
            );
        }
        if !closed {
            eprintln!(
                "note: reached end of string without closing it: {:?}",
                string
            );
        }
        (value, index)
    } else {
        let end_offset = match bytes[offset..].iter().position(|b| b.is_ascii_whitespace()) {
            Some(i) => offset + i,
            None => bytes.len(),
        };
        (bytes[offset..end_offset].to_vec(), end_offset)
    };

    *string = &string[end_offset..];
    String::from_utf8(value).ok()
}

/// Get the absolute path out of value given the root and the path of the file being processed.
pub fn get_abs_path(root: &PathBuf, path: Option<&PathBuf>, value: &str) -> PathBuf {
    if value.starts_with('/') {
        let mut p = root.clone();
        p.push(&value[1..]);
        p
    } else {
        let mut p = path.unwrap_or(root).clone();
        p.push(value);
        p
    }
}

/// Replace's `path`'s `source` root with `destination`. Panics if `path` does not start with `source`.
///
/// Rust's path (and `OsString`) manipulation is pretty lacking, so the method falls back to `String`.
pub fn replace_root(source: &String, destination: &String, path: &String) -> PathBuf {
    assert!(path.starts_with(source));
    let rel = &path[source.len() + 1..]; // +1 to skip path separator
    let mut dir = PathBuf::from(&destination);
    dir.push(rel);
    dir
}

pub fn parse_opt_date(path: &PathBuf, created: bool, string: Option<&String>) -> NaiveDate {
    match string {
        Some(s) => match NaiveDate::parse_from_str(s, crate::DATE_FMT) {
            Ok(d) => return d,
            Err(_) => eprintln!("note: invalid date value: {:?}", s),
        },
        None => {}
    }

    match fs::metadata(&path) {
        Ok(meta) => {
            if created {
                match meta.created() {
                    Ok(date) => {
                        return NaiveDateTime::from_timestamp(
                            date.duration_since(UNIX_EPOCH).unwrap().as_secs() as i64,
                            0,
                        )
                        .date()
                    }
                    Err(_) => eprintln!("note: failed to fetch creation date for file: {:?}", path),
                }
            } else {
                match meta.modified() {
                    Ok(date) => {
                        return NaiveDateTime::from_timestamp(
                            date.duration_since(UNIX_EPOCH).unwrap().as_secs() as i64,
                            0,
                        )
                        .date()
                    }
                    Err(_) => eprintln!(
                        "note: failed to fetch modification date for file: {:?}",
                        path
                    ),
                }
            }
        }
        Err(_) => eprintln!("note: failed to fetch metadata for file: {:?}", path),
    }

    chrono::Local::today().naive_local()
}
