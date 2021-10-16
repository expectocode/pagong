use std::path::{Path, PathBuf};

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
            } else if bytes[index] == b'\\' {
                escape = true;
            } else if bytes[index] == b'"' {
                closed = true;
                index += 1;
                break;
            } else {
                value.push(bytes[index]);
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
pub fn get_abs_path(root: &Path, path: &Path, value: &str) -> PathBuf {
    if let Some(absolute) = value.strip_prefix('/') {
        let mut p = root.to_path_buf();
        p.push(absolute);
        p
    } else {
        let mut p = path.parent().unwrap().to_owned();
        p.push(value);
        p
    }
}

/// Replace's `path`'s `source` root with `destination`. Panics if `path` does not start with `source`.
///
/// Rust's path (and `OsString`) manipulation is pretty lacking, so the method falls back to `String`.
pub fn replace_root(source: &str, destination: &str, path: &str) -> PathBuf {
    assert!(path.starts_with(source));
    let rel = &path[(source.len() + 1).min(path.len())..]; // +1 to skip path separator
    let mut dir = PathBuf::from(&destination);
    dir.push(rel);
    dir
}

pub fn path_to_uri(root: &Path, path: &Path) -> String {
    replace_root(
        &root.to_str().unwrap().to_owned(),
        &std::path::MAIN_SEPARATOR.to_string(),
        &path.to_str().unwrap().to_owned(),
    )
    .to_str()
    .unwrap()
    .replace(std::path::MAIN_SEPARATOR, "/")
}

pub fn get_relative_uri(relative_to: &str, uri: &str) -> String {
    let relative_to = relative_to.as_bytes();
    let uri = uri.as_bytes();

    let mut count_after = relative_to.len();
    let mut last_shared_slash = 0;
    for i in 0..relative_to.len().max(uri.len()) {
        if relative_to.get(i) != uri.get(i) {
            count_after = i;
            break;
        } else if let Some(b'/') = uri.get(i) {
            last_shared_slash = i;
        }
    }

    let up_count = relative_to
        .iter()
        .skip(count_after)
        .filter(|c| **c == b'/')
        .count();

    let mut result = String::new();
    (0..up_count).for_each(|_| result.push_str("../"));
    uri[last_shared_slash + 1..]
        .iter()
        .for_each(|c| result.push(*c as _));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    mod parse_value {
        use super::*;

        #[test]
        fn simple() {
            let mut string = "simple";
            assert_eq!(parse_next_value(&mut string), Some("simple".to_owned()));
        }

        #[test]
        fn quoted() {
            let mut string = "\"quoted\"";
            assert_eq!(parse_next_value(&mut string), Some("quoted".to_owned()));
        }

        #[test]
        fn good_escape() {
            let mut string = "\"good\\\" \\\"escape\"";
            assert_eq!(
                parse_next_value(&mut string),
                Some("good\" \"escape".to_owned())
            );
        }

        #[test]
        fn bad_escape() {
            let mut string = "\"bad\\_escape\"";
            assert_eq!(parse_next_value(&mut string), Some("bad_escape".to_owned()));
        }

        #[test]
        fn unterminated() {
            let mut string = "\"unterminated";
            assert_eq!(
                parse_next_value(&mut string),
                Some("unterminated".to_owned())
            );
        }

        #[test]
        fn multiple() {
            let mut string = " simple \t\"quoted\" \n \"\\\"escapes\\\\\" \n\t \r simple";
            let string = &mut string;
            let mut values = Vec::new();
            while let Some(value) = parse_next_value(string) {
                values.push(value);
            }

            assert_eq!(values, vec!["simple", "quoted", "\"escapes\\", "simple"]);
        }
    }
}
