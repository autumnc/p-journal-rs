/// Get display width of a single character (CJK characters = 2, others = 1).
/// Matches the Unicode range logic from the Python version.
pub fn char_width(ch: char) -> usize {
    let cp = ch as u32;
    if (0x1100..=0x115F).contains(&cp)
        || (0x2E80..=0x303E).contains(&cp)
        || (0x3040..=0x9FFF).contains(&cp)
        || (0xAC00..=0xD7A3).contains(&cp)
        || (0xF900..=0xFAFF).contains(&cp)
        || (0xFE30..=0xFE6F).contains(&cp)
        || (0xFF01..=0xFF60).contains(&cp)
        || (0xFFE0..=0xFFE6).contains(&cp)
        || (0x20000..=0x2FFFD).contains(&cp)
        || (0x30000..=0x3FFFD).contains(&cp)
    {
        2
    } else {
        1
    }
}

/// Get display width of a string.
pub fn string_width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

fn byte_at_char_pos(chars: &[(usize, char)], pos: usize) -> usize {
    if pos < chars.len() {
        chars[pos].0
    } else {
        chars.last().map(|(i, ch)| i + ch.len_utf8()).unwrap_or(0)
    }
}

fn is_cjk_line_break_char(ch: char) -> bool {
    char_width(ch) == 2
}

/// Word wrap a single line, returning (start, end) byte-index segments.
/// Supports CJK character widths for display-width calculations.
pub fn wrap_line(line: &str, display_width: usize) -> Vec<(usize, usize)> {
    if display_width == 0 {
        return vec![(0, line.len())];
    }
    if line.is_empty() {
        return vec![(0, 0)];
    }

    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let mut segments = Vec::new();
    let mut seg_start = 0usize;

    while seg_start < chars.len() {
        let mut pos = seg_start;
        let mut current_width = 0usize;
        let mut last_break: Option<(usize, usize)> = None;

        while pos < chars.len() {
            let ch = chars[pos].1;
            let cw = char_width(ch);
            if current_width > 0 && current_width + cw > display_width {
                break;
            }

            current_width += cw;
            pos += 1;

            if ch == ' ' {
                last_break = Some((pos, pos));
            } else if is_cjk_line_break_char(ch) {
                last_break = Some((pos, pos));
            }
        }

        if pos >= chars.len() {
            segments.push((chars[seg_start].0, byte_at_char_pos(&chars, chars.len())));
            break;
        }

        let (break_end, next_start) = last_break
            .filter(|(end, next)| *end > seg_start && *next > seg_start)
            .unwrap_or((pos, pos));

        segments.push((chars[seg_start].0, byte_at_char_pos(&chars, break_end)));
        seg_start = next_start;
    }

    if segments.is_empty() {
        vec![(0, 0)]
    } else {
        segments
    }
}

/// A visual row representing one screen line of wrapped text.
#[derive(Debug, Clone)]
pub struct VisualRow {
    pub logical_line: usize,
    pub start_byte: usize,
    pub end_byte: usize,
}

/// Build wrap map: for each logical line, produce visual rows.
pub fn build_wrap_map(lines: &[String], display_width: usize) -> Vec<VisualRow> {
    let mut vrows = Vec::new();
    for (li, line) in lines.iter().enumerate() {
        let segs = wrap_line(line, display_width);
        for (start, end) in segs {
            vrows.push(VisualRow {
                logical_line: li,
                start_byte: start,
                end_byte: end,
            });
        }
    }
    vrows
}

/// Check if a char is in the CJK unified ideograph range (used for word counting).
fn is_cjk_char(ch: char) -> bool {
    let cp = ch as u32;
    (0x4E00..=0x9FFF).contains(&cp)
        || (0x3000..=0x303F).contains(&cp)
        || (0xFF00..=0xFFEF).contains(&cp)
}

/// Count words: Chinese characters counted individually, English words as word tokens.
pub fn word_count(lines: &[String]) -> usize {
    let mut total = 0;

    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let mut chinese_chars = 0;
        let mut english_words = 0;
        let mut in_english = false;

        for ch in line.chars() {
            if is_cjk_char(ch) {
                chinese_chars += 1;
                in_english = false;
            } else if ch.is_ascii_alphabetic() {
                if !in_english {
                    english_words += 1;
                    in_english = true;
                }
            } else {
                in_english = false;
            }
        }
        total += chinese_chars + english_words;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::{string_width, wrap_line};

    fn wrapped_text(line: &str, width: usize) -> Vec<&str> {
        wrap_line(line, width)
            .into_iter()
            .map(|(start, end)| &line[start..end])
            .collect()
    }

    #[test]
    fn wraps_between_cjk_chars() {
        assert_eq!(wrapped_text("中文字符", 4), vec!["中文", "字符"]);
    }

    #[test]
    fn cjk_breaks_after_space_in_mixed_text() {
        assert_eq!(wrapped_text("abc 中文def", 6), vec!["abc 中", "文def"]);
    }

    #[test]
    fn wrapped_segments_stay_within_width_when_possible() {
        for segment in wrapped_text("hello 中文 world", 8) {
            assert!(string_width(segment) <= 8);
        }
    }
}
