//! Small, bounded parsers for the metadata subset Viz needs.
//!
//! These are intentionally not general TOML or JSON implementations. They
//! extract string metadata while keeping the release binary dependency-free.

use crate::error::{Error, Result};
use std::fs;
use std::path::Path;

pub(crate) const MAX_MANIFEST_BYTES: u64 = 1_048_576;

pub(crate) fn read_manifest(path: &Path) -> Result<String> {
    let metadata = fs::metadata(path).map_err(|error| Error::io("inspect", path, error))?;
    if metadata.len() > MAX_MANIFEST_BYTES {
        return Err(Error::Inspection(format!(
            "manifest exceeds the 1 MiB inspection limit: {}",
            path.display()
        )));
    }
    fs::read_to_string(path).map_err(|error| Error::io("read", path, error))
}

pub(crate) fn toml_value_in_section(
    input: &str,
    wanted_section: &str,
    key: &str,
) -> Option<String> {
    toml_pairs_in_section(input, wanted_section)
        .into_iter()
        .find_map(|(candidate, value, _)| (candidate == key).then_some(value))
}

pub(crate) fn line_containing_in_section(
    input: &str,
    wanted_section: &str,
    key: &str,
) -> Option<usize> {
    toml_pairs_in_section(input, wanted_section)
        .into_iter()
        .find_map(|(candidate, _, line)| (candidate == key).then_some(line))
}

pub(crate) fn toml_pairs_in_section(
    input: &str,
    wanted_section: &str,
) -> Vec<(String, String, usize)> {
    let mut section = "";
    let mut pairs = Vec::new();
    for (index, raw_line) in input.lines().enumerate() {
        let line = raw_line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            section = line.trim_matches(['[', ']']).trim();
            continue;
        }
        if section != wanted_section || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim().trim_matches(['\'', '"']);
            let value = value.trim().split('#').next().unwrap_or("").trim();
            let value = value.trim_matches(['\'', '"']);
            if !key.is_empty() && !value.is_empty() {
                pairs.push((key.to_owned(), value.to_owned(), index + 1));
            }
        }
    }
    pairs
}

pub(crate) fn json_string_field(input: &str, wanted_key: &str) -> Option<String> {
    json_object_members(input).into_iter().find_map(|member| {
        (member.key == wanted_key)
            .then(|| parse_json_string(member.value.trim_start()).map(|(value, _)| value))
            .flatten()
    })
}

pub(crate) fn json_object_keys(input: &str, wanted_key: &str) -> Vec<String> {
    let Some(member) = json_object_members(input)
        .into_iter()
        .find(|member| member.key == wanted_key)
    else {
        return Vec::new();
    };
    json_object_members(member.value)
        .into_iter()
        .map(|member| member.key)
        .collect()
}

pub(crate) fn line_containing_json_key(input: &str, key: &str) -> Option<usize> {
    let needle = format!("\"{key}\"");
    input
        .lines()
        .position(|line| line.contains(&needle))
        .map(|index| index + 1)
}

#[derive(Debug)]
struct JsonMember<'a> {
    key: String,
    value: &'a str,
}

fn json_object_members(mut input: &str) -> Vec<JsonMember<'_>> {
    input = input.trim_start();
    let Some(mut cursor) = input.find('{').map(|index| index + 1) else {
        return Vec::new();
    };
    let mut members = Vec::new();
    loop {
        cursor += input[cursor..]
            .find(|character: char| !character.is_whitespace() && character != ',')
            .unwrap_or(input.len() - cursor);
        if cursor >= input.len() || input[cursor..].starts_with('}') {
            break;
        }
        let Some((key, key_bytes)) = parse_json_string(&input[cursor..]) else {
            break;
        };
        cursor += key_bytes;
        cursor += input[cursor..]
            .find(|character: char| !character.is_whitespace())
            .unwrap_or(input.len() - cursor);
        if !input[cursor..].starts_with(':') {
            break;
        }
        cursor += 1;
        cursor += input[cursor..]
            .find(|character: char| !character.is_whitespace())
            .unwrap_or(input.len() - cursor);
        let value_start = cursor;
        let Some(value_bytes) = json_value_length(&input[value_start..]) else {
            break;
        };
        members.push(JsonMember {
            key,
            value: &input[value_start..value_start + value_bytes],
        });
        cursor = value_start + value_bytes;
    }
    members
}

fn json_value_length(input: &str) -> Option<usize> {
    let first = input.chars().next()?;
    if first == '"' {
        return parse_json_string(input).map(|(_, bytes)| bytes);
    }
    if matches!(first, '{' | '[') {
        let mut expected_closers = Vec::new();
        let mut in_string = false;
        let mut escaped = false;
        for (offset, character) in input.char_indices() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if character == '\\' {
                    escaped = true;
                } else if character == '"' {
                    in_string = false;
                }
            } else if character == '"' {
                in_string = true;
            } else if character == '{' {
                expected_closers.push('}');
            } else if character == '[' {
                expected_closers.push(']');
            } else if matches!(character, '}' | ']') {
                if expected_closers.pop() != Some(character) {
                    return None;
                }
                if expected_closers.is_empty() {
                    return Some(offset + character.len_utf8());
                }
            }
        }
        return None;
    }
    Some(input.find([',', '}']).unwrap_or(input.len()))
}

fn parse_json_string(input: &str) -> Option<(String, usize)> {
    if !input.starts_with('"') {
        return None;
    }
    let mut output = String::new();
    let mut escaped = false;
    for (offset, character) in input[1..].char_indices() {
        if escaped {
            match character {
                '"' | '\\' | '/' => output.push(character),
                'b' => output.push('\u{0008}'),
                'f' => output.push('\u{000c}'),
                'n' => output.push('\n'),
                'r' => output.push('\r'),
                't' => output.push('\t'),
                _ => return None,
            }
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            return Some((output, offset + 2));
        } else {
            output.push(character);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_keys_do_not_include_nested_values() {
        let input = r#"{"scripts":{"demo":"node x"},"nested":{"wrong":"value"}}"#;
        assert_eq!(json_object_keys(input, "scripts"), vec!["demo"]);
    }

    #[test]
    fn top_level_name_wins_over_nested_name() {
        let input = r#"{"nested":{"name":"wrong"},"name":"right"}"#;
        assert_eq!(json_string_field(input, "name").as_deref(), Some("right"));
    }

    #[test]
    fn object_scanner_ignores_braces_inside_strings() {
        let input = r#"{"scripts":{"demo":"echo } {"},"name":"safe"}"#;
        assert_eq!(json_object_keys(input, "scripts"), vec!["demo"]);
        assert_eq!(json_string_field(input, "name").as_deref(), Some("safe"));
    }

    #[test]
    fn object_scanner_handles_mixed_nested_containers() {
        let input = r#"{"metadata":[{"brackets":"]"}],"name":"safe"}"#;
        assert_eq!(json_string_field(input, "name").as_deref(), Some("safe"));
    }
}
