//! Unicode-aware text helpers: display width, truncation, word wrapping, and
//! human-friendly time formatting.

use chrono::{DateTime, Timelike, Utc};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Display width of a string in terminal cells (handles wide CJK chars).
pub fn width(s: &str) -> usize {
    s.width()
}

/// Truncate a string to at most `max` display columns, appending an ellipsis
/// when truncated. Newlines are replaced with spaces first.
pub fn truncate(s: &str, max: usize) -> String {
    let flat: String = s
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect();
    if width(&flat) <= max {
        return flat;
    }
    if max == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut w = 0usize;
    for c in flat.graphemes(true) {
        let cw = width(c);
        if w + cw > max - 1 {
            break;
        }
        out.push_str(c);
        w += cw;
    }
    out.push('…');
    out
}

/// Word-wrap `text` into lines of at most `max` display columns. Words longer
/// than the width are hard-broken. Empty input yields an empty vec.
pub fn wrap(text: &str, max: usize) -> Vec<String> {
    if max == 0 {
        return vec![];
    }
    let mut lines: Vec<String> = Vec::new();
    for raw_line in text.split('\n') {
        let mut current = String::new();
        let mut cur_w = 0usize;
        for word in raw_line.split(' ') {
            // Hard-break over-long words.
            if width(word) > max {
                if !current.is_empty() {
                    lines.push(std::mem::take(&mut current));
                }
                let mut chunk = String::new();
                let mut chunk_w = 0usize;
                for c in word.graphemes(true) {
                    let cw = width(c);
                    if chunk_w + cw > max {
                        lines.push(std::mem::take(&mut chunk));
                        chunk_w = 0;
                    }
                    chunk.push_str(c);
                    chunk_w += cw;
                }
                current = chunk;
                cur_w = chunk_w;
                continue;
            }
            let add_w = if current.is_empty() {
                width(word)
            } else {
                cur_w + 1 + width(word)
            };
            if add_w > max && !current.is_empty() {
                lines.push(std::mem::take(&mut current));
                cur_w = 0;
            }
            if current.is_empty() {
                current.push_str(word);
                cur_w = width(word);
            } else {
                current.push(' ');
                current.push_str(word);
                cur_w += 1 + width(word);
            }
        }
        lines.push(current);
    }
    lines
}

/// "just now", "42s", "5m", "3h", "2d", or a short date like "Jan 5".
pub fn relative_time(dt: DateTime<Utc>) -> String {
    let now = Utc::now();
    let secs = now.signed_duration_since(dt).num_seconds().max(0);
    if secs < 60 {
        "just now".into()
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86_400 {
        format!("{}h", secs / 3600)
    } else if secs < 7 * 86_400 {
        format!("{}d", secs / 86_400)
    } else {
        dt.format("%b %e").to_string()
    }
}

/// Local wall-clock time "14:32".
pub fn time_of_day(dt: DateTime<Utc>) -> String {
    let local = dt.with_timezone(&chrono::Local);
    format!("{:02}:{:02}", local.hour(), local.minute())
}

/// Day label for chat separators: "Today", "Yesterday", or "Mon, Jan 5".
pub fn day_label(dt: DateTime<Utc>) -> String {
    let local = dt.with_timezone(&chrono::Local);
    let today = chrono::Local::now().date_naive();
    let d = local.naive_local().date();
    if d == today {
        "Today".into()
    } else if d == today - chrono::TimeDelta::days(1) {
        "Yesterday".into()
    } else {
        local.format("%a, %b %e").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_basic() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 8), "hello w…");
        assert_eq!(truncate("", 5), "");
        assert_eq!(truncate("hi", 0), "");
    }

    #[test]
    fn truncate_wide_chars() {
        // Each CJK char is 2 columns.
        assert_eq!(width("日本語"), 6);
        assert_eq!(truncate("日本語", 5), "日本…");
    }

    #[test]
    fn wrap_basic() {
        let lines = wrap("aa bb cc dd", 7);
        assert_eq!(lines, vec!["aa bb", "cc dd"]);
    }

    #[test]
    fn wrap_long_word_hard_break() {
        let lines = wrap("abcdefghij", 4);
        assert_eq!(lines, vec!["abcd", "efgh", "ij"]);
    }

    #[test]
    fn wrap_newlines_preserved() {
        let lines = wrap("one\ntwo three", 10);
        assert_eq!(lines, vec!["one", "two three"]);
    }

    #[test]
    fn relative_time_recent() {
        let dt = Utc::now() - chrono::TimeDelta::seconds(10);
        assert_eq!(relative_time(dt), "just now");
    }

    #[test]
    fn time_format() {
        let dt = DateTime::parse_from_rfc3339("2024-01-02T14:32:00Z")
            .unwrap()
            .with_timezone(&Utc);
        // Local offset may shift the hour, but format must be H(H):MM.
        let s = time_of_day(dt);
        let (h, m) = s.split_once(':').expect("missing colon");
        assert!((1..=2).contains(&h.len()), "bad hour in {s:?}");
        assert!(h.chars().all(|c| c.is_ascii_digit()));
        assert_eq!(m.len(), 2);
        assert!(m.chars().all(|c| c.is_ascii_digit()));
    }
}
