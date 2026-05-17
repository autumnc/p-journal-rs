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
    let len = chars.len();
    let mut segments = Vec::new();
    let mut pos = 0;
    let mut current_width = 0;
    let mut seg_start = 0;

    while pos < len {
        let ch = chars[pos].1;
        let cw = char_width(ch);

        if current_width + cw > display_width {
            if ch == ' ' {
                segments.push((chars[seg_start].0, chars[pos].0));
                pos += 1;
                seg_start = pos;
                current_width = 0;
            } else {
                let mut has_space = false;
                let mut break_pos = pos;
                for i in seg_start..pos {
                    if chars[i].1 == ' ' {
                        has_space = true;
                        break_pos = i;
                    }
                }

                if has_space && break_pos > seg_start {
                    segments.push((chars[seg_start].0, chars[break_pos].0));
                    pos = break_pos + 1;
                    seg_start = pos;
                    current_width = if pos < len {
                        let s: String = chars[seg_start..=pos].iter().map(|(_, c)| c).collect();
                        string_width(&s)
                    } else {
                        0
                    };
                } else {
                    segments.push((chars[seg_start].0, chars[pos].0));
                    seg_start = pos;
                    current_width = cw;
                }
            }
            if seg_start == pos {
                pos += 1;
            }
            if seg_start < pos {
                continue;
            }
        } else {
            current_width += cw;
            pos += 1;
        }
    }

    if seg_start < len {
        let end = chars.last().map(|(i, c)| i + c.len_utf8()).unwrap_or(0);
        segments.push((chars[seg_start].0, end));
    } else if seg_start == len && segments.is_empty() {
        segments.push((0, 0));
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
