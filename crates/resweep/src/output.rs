//! Printing, matched to the tool being replaced rather than to taste.
//!
//! The shape of what comes out is the contract: the guidance reads it, the
//! suites compare it, and the recorded reference is judged byte for byte. Two
//! details are easy to get wrong and invisible until something downstream
//! breaks, so both are handled here in one place.

use serde_json::Value;

/// Two-space indent, keys in insertion order, and every non-ASCII character
/// escaped.
///
/// The escaping is the detail. Python's json.dumps escapes non-ASCII by
/// default and serde_json does not, so a note or a path with an accent in it
/// would come out as different bytes for the same content.
pub fn json(value: &Value) -> String {
    escape_non_ascii(&serde_json::to_string_pretty(value).expect("a printable payload"))
}

fn escape_non_ascii(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii() {
            out.push(c);
        } else {
            // Surrogate pairs for anything outside the basic plane, which is
            // what Python emits.
            let mut buf = [0u16; 2];
            for unit in c.encode_utf16(&mut buf) {
                out.push_str(&format!("\\u{unit:04x}"));
            }
        }
    }
    out
}

/// One line, with a space after every colon and comma.
///
/// This is what Python's json.dumps produces by default, and the string it
/// produces is stored in the ledger and echoed back by `status`. serde_json's
/// compact form has no spaces, so the same content would be different bytes on
/// disk and in the output.
pub fn json_compact(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let inner: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("{}: {}", serde_json::to_string(k).unwrap_or_default(), json_compact(v)))
                .collect();
            format!("{{{}}}", inner.join(", "))
        }
        Value::Array(items) => {
            let inner: Vec<String> = items.iter().map(json_compact).collect();
            format!("[{}]", inner.join(", "))
        }
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

/// The wall-clock stamp every row carries: seconds precision, UTC, offset
/// spelled out rather than abbreviated to Z, which is what the ledger already
/// holds and what the reference records.
pub fn now() -> String {
    stamp(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
    )
}

fn stamp(epoch_seconds: i64) -> String {
    let days = epoch_seconds.div_euclid(86_400);
    let secs = epoch_seconds.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}+00:00",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

/// Howard Hinnant's days-to-civil algorithm. Written out rather than pulled in
/// as a dependency: one date format is needed and a calendar crate is a large
/// surface for it.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as i64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Wrap a paragraph the way Python's textwrap.fill does at the same width.
///
/// Whitespace is collapsed, words are separated by single spaces, and a word
/// longer than the width is broken rather than allowed to overhang. The report
/// puts every note through this, so a different wrapping is a different report.
pub fn fill(text: &str, width: usize) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let mut word = word;
        loop {
            let room = if current.is_empty() { width } else { width - current.chars().count() - 1 };
            if word.chars().count() <= room {
                if !current.is_empty() {
                    current.push(' ');
                }
                current.push_str(word);
                break;
            }
            if current.is_empty() {
                // Longer than a whole line: break it, as textwrap does by
                // default rather than letting it overhang.
                let cut = word
                    .char_indices()
                    .nth(width)
                    .map(|(i, _)| i)
                    .unwrap_or(word.len());
                let (head, tail) = word.split_at(cut);
                lines.push(head.to_string());
                word = tail;
            } else {
                lines.push(std::mem::take(&mut current));
            }
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_non_ascii_note_is_escaped_the_way_python_escapes_it() {
        let value = json!({ "note": "café" });
        assert!(json_of(&value).contains("caf\\u00e9"), "{}", json_of(&value));
    }

    #[test]
    fn an_emoji_becomes_a_surrogate_pair() {
        let value = json!({ "note": "\u{1f600}" });
        assert!(json_of(&value).contains("\\ud83d\\ude00"), "{}", json_of(&value));
    }

    fn json_of(v: &Value) -> String {
        json(v)
    }

    #[test]
    fn keys_stay_in_the_order_they_were_inserted() {
        let mut map = serde_json::Map::new();
        map.insert("zebra".into(), json!(1));
        map.insert("apple".into(), json!(2));
        let printed = json(&Value::Object(map));
        assert!(
            printed.find("zebra").unwrap() < printed.find("apple").unwrap(),
            "{printed}"
        );
    }

    #[test]
    fn the_indent_is_two_spaces() {
        let printed = json(&json!({ "a": { "b": 1 } }));
        assert_eq!(printed, "{\n  \"a\": {\n    \"b\": 1\n  }\n}");
    }

    #[test]
    fn an_empty_container_stays_on_one_line() {
        assert_eq!(json(&json!({ "a": {}, "b": [] })), "{\n  \"a\": {},\n  \"b\": []\n}");
    }

    #[test]
    fn the_compact_form_spaces_the_way_python_does() {
        let value = json!({ ".js": 62, ".css": 3 });
        let printed = json_compact(&value);
        assert!(printed.contains("\": "), "{printed}");
        assert!(printed.contains(", "), "{printed}");
        assert_eq!(json_compact(&json!({})), "{}");
    }

    #[test]
    fn a_known_instant_formats_the_way_the_ledger_records_it() {
        assert_eq!(stamp(0), "1970-01-01T00:00:00+00:00");
        assert_eq!(stamp(1_789_257_600), "2026-09-13T00:00:00+00:00");
    }
}
