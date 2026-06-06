use crate::cjk::{build_wrap_map, string_width, word_count, VisualRow};
use crate::config::{load_config, tab_width};
use crate::deepseek::generate_ai_prompt;
use crate::flomo::send_to_flomo;
use crate::ui::browser::{format_status_bar, show_message};
use chrono::Local;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use ratatui_markdown::{RichTextTheme, ThemeConfig};
use std::io;

pub enum EditorResult {
    Commit(String, Option<String>),
    Cancel,
}

pub fn journal_editor(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stderr>>,
    prompt_text: Option<String>,
) -> io::Result<EditorResult> {
    let mut lines: Vec<String> = vec![String::new()];
    let mut cy: usize = 0; // logical line
    let mut cx: usize = 0; // byte offset in current line
    let mut scroll_y: usize = 0;
    let mut target_screen_cx: Option<usize> = None;
    let mut current_prompt = prompt_text;

    let accent = Style::default().fg(Color::Yellow);
    let md_theme = markdown_theme();

    // Compute prompt display lines
    let get_prompt_info = |prompt: &Option<String>, term_w: usize| -> (Vec<String>, usize) {
        match prompt {
            Some(ref p) => {
                let wrapped = textwrap::fill(p, term_w.saturating_sub(6));
                let plines: Vec<String> = wrapped.lines().map(String::from).collect();
                let ph = plines.len() + 3; // blank + lines + blank + separator
                (plines, ph)
            }
            None => (Vec::new(), 0),
        }
    };

    loop {
        let term_size = terminal.size()?;
        let w = term_size.width as usize;
        let h = term_size.height as usize;
        let (prompt_lines, prompt_h) = get_prompt_info(&current_prompt, w);
        let text_h = h.saturating_sub(2 + prompt_h);

        // Clamp cursor
        cy = cy.min(lines.len().saturating_sub(1));
        cx = cx.min(lines[cy].len());

        // Build wrap map
        let vrows = build_wrap_map(&lines, w);

        // Find cursor visual position
        let (vi_cursor, scx_cursor) = find_cursor_visual(&vrows, &lines, cy, cx);

        // Scroll
        if vi_cursor < scroll_y {
            scroll_y = vi_cursor;
        }
        if vi_cursor >= scroll_y + text_h {
            scroll_y = vi_cursor.saturating_sub(text_h) + 1;
        }
        scroll_y = scroll_y.min(vrows.len().saturating_sub(text_h).max(0));

        // Render
        let cfg = load_config();
        let md_enabled = cfg.markdown_enabled;
        terminal.draw(|f| {
            render_editor(
                f,
                &lines,
                &vrows,
                &prompt_lines,
                prompt_h,
                text_h,
                scroll_y,
                vi_cursor,
                scx_cursor,
                &current_prompt,
                cy,
                &accent,
                md_enabled,
                &md_theme,
            );
        })?;

        // Input
        let ev = event::read()?;
        let Event::Key(key) = ev else { continue };
        if key.kind == KeyEventKind::Release {
            continue;
        }

        let mut continue_sticky = false;

        match key.code {
            // Navigation
            KeyCode::Up => {
                if vi_cursor > 0 {
                    if target_screen_cx.is_none() {
                        target_screen_cx = Some(scx_cursor);
                    }
                    let tcx = target_screen_cx.unwrap_or(0);
                    for vi2 in (0..vi_cursor).rev() {
                        if vrows[vi2].logical_line != vrows[vi_cursor].logical_line || vi2 == 0 {
                            let vr = &vrows[vi2];
                            cy = vr.logical_line;
                            cx = byte_at_screen_pos(&lines[vr.logical_line], vr.start_byte, vr.end_byte, tcx);
                            break;
                        }
                    }
                }
                continue_sticky = true;
            }
            KeyCode::Down => {
                if vi_cursor < vrows.len().saturating_sub(1) {
                    if target_screen_cx.is_none() {
                        target_screen_cx = Some(scx_cursor);
                    }
                    let tcx = target_screen_cx.unwrap_or(0);
                    let current_li = vrows[vi_cursor].logical_line;
                    for vi2 in (vi_cursor + 1)..vrows.len() {
                        if vrows[vi2].logical_line != current_li {
                            let vr = &vrows[vi2];
                            cy = vr.logical_line;
                            cx = byte_at_screen_pos(&lines[vr.logical_line], vr.start_byte, vr.end_byte, tcx);
                            break;
                        }
                    }
                }
                continue_sticky = true;
            }
            KeyCode::Left => {
                if cx > 0 {
                    // Move back one char (byte offset)
                    if let Some((prev, _)) = lines[cy].char_indices().rev().find(|(i, _)| *i < cx) {
                        cx = prev;
                    } else {
                        cx = 0;
                    }
                } else if cy > 0 {
                    cy -= 1;
                    cx = lines[cy].len();
                }
            }
            KeyCode::Right => {
                if cx < lines[cy].len() {
                    // Move forward one char
                    if let Some((next, _)) = lines[cy].char_indices().find(|(i, _)| *i > cx) {
                        cx = next;
                    } else {
                        cx = lines[cy].len();
                    }
                } else if cy < lines.len().saturating_sub(1) {
                    cy += 1;
                    cx = 0;
                }
            }
            KeyCode::Home => {
                let vr = find_vrow_at_cursor(&vrows, cy, cx);
                cx = vr.start_byte;
            }
            KeyCode::End => {
                let vr = find_vrow_at_cursor(&vrows, cy, cx);
                cx = vr.end_byte;
            }
            KeyCode::PageUp => {
                let target_vi = vi_cursor.saturating_sub(text_h);
                if target_screen_cx.is_none() {
                    target_screen_cx = Some(scx_cursor);
                }
                if target_vi < vrows.len() {
                    let vr = &vrows[target_vi];
                    cy = vr.logical_line;
                    cx = vr.start_byte;
                }
            }
            KeyCode::PageDown => {
                let target_vi = (vi_cursor + text_h).min(vrows.len().saturating_sub(1));
                if target_screen_cx.is_none() {
                    target_screen_cx = Some(scx_cursor);
                }
                if target_vi < vrows.len() {
                    let vr = &vrows[target_vi];
                    cy = vr.logical_line;
                    cx = vr.start_byte;
                }
            }

            // Editing
            KeyCode::Backspace => {
                if cx > 0 {
                    // Remove char before cursor
                    if let Some((prev, _)) = lines[cy].char_indices().rev().find(|(i, _)| *i < cx) {
                        lines[cy].remove(prev);
                        cx = prev;
                    }
                } else if cy > 0 {
                    cx = lines[cy - 1].len();
                    let rest = lines.remove(cy);
                    cy -= 1;
                    lines[cy].push_str(&rest);
                }
            }
            KeyCode::Delete => {
                if cx < lines[cy].len() {
                    // Remove char at cursor
                    if let Some((next, _)) = lines[cy].char_indices().find(|(i, _)| *i >= cx) {
                        lines[cy].remove(next);
                    }
                } else if cy < lines.len().saturating_sub(1) {
                    let rest = lines.remove(cy + 1);
                    lines[cy].push_str(&rest);
                }
            }
            KeyCode::Enter => {
                let rest = lines[cy][cx..].to_string();
                lines[cy].truncate(cx);
                cy += 1;
                lines.insert(cy, rest);
                cx = 0;
            }
            KeyCode::Tab => {
                let spaces = " ".repeat(tab_width());
                lines[cy].insert_str(cx, &spaces);
                cx += tab_width();
            }
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let text = lines.join("\n").trim().to_string();
                if text.is_empty() {
                    return Ok(EditorResult::Cancel);
                }
                return Ok(EditorResult::Commit(text, current_prompt.clone()));
            }
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let text = lines.join("\n").trim().to_string();
                if text.is_empty() {
                    return Ok(EditorResult::Cancel);
                }
                if confirm_abandon(terminal)? {
                    return Ok(EditorResult::Cancel);
                }
            }
            KeyCode::Esc => {
                let text = lines.join("\n").trim().to_string();
                if text.is_empty() {
                    return Ok(EditorResult::Cancel);
                }
                if confirm_abandon(terminal)? {
                    return Ok(EditorResult::Cancel);
                }
            }
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let cfg = load_config();
                if cfg.deepseek_api_key.is_empty() {
                    show_message(terminal, "请先在设置中配置 Deepseek API Key", 2)?;
                } else {
                    show_message(terminal, "AI 生成提示词中，请稍候...", 1)?;
                    terminal.flush()?;
                    let result = generate_ai_prompt(
                        &cfg.deepseek_api_key,
                        &cfg.personal_experience,
                        &cfg.personal_hobbies,
                        &cfg.personal_recent_status,
                        current_prompt.as_deref(),
                    );
                    match result {
                        Some(text) => {
                            current_prompt = Some(text);
                            show_message(terminal, "✓ AI 提示词已生成", 1)?;
                        }
                        None => {
                            show_message(terminal, "✗ 生成失败，请检查 API Key 和网络连接", 2)?;
                        }
                    }
                }
                continue;
            }
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let text = lines.join("\n").trim().to_string();
                if text.is_empty() {
                    show_message(terminal, "日记内容为空，无法发送", 1)?;
                    continue;
                }
                show_message(terminal, "正在发送到 Flomo...", 1)?;
                let mut cfg = load_config();
                let (_, msg) = send_to_flomo(&text, &mut cfg);
                show_message(terminal, &msg, 2)?;
                return Ok(EditorResult::Commit(text, current_prompt.clone()));
            }

            // Printable chars (including CJK)
            KeyCode::Char(c) => {
                lines[cy].insert(cx, c);
                cx += c.len_utf8();
            }

            _ => {}
        }

        if !continue_sticky {
            target_screen_cx = None;
        }
    }
}

fn find_cursor_visual(
    vrows: &[VisualRow],
    lines: &[String],
    cy: usize,
    cx: usize,
) -> (usize, usize) {
    for (vi, vr) in vrows.iter().enumerate() {
        if vr.logical_line == cy && vr.start_byte <= cx && cx <= vr.end_byte {
            if cx == vr.end_byte && vr.end_byte > vr.start_byte {
                if vi + 1 < vrows.len() && vrows[vi + 1].logical_line == cy {
                    continue;
                }
            }
            let prefix = &lines[cy][vr.start_byte..cx];
            return (vi, string_width(prefix));
        }
    }
    if let Some(vr) = vrows.last() {
        let li = vr.logical_line;
        if li < lines.len() {
            let prefix = &lines[li][vr.start_byte..cx.min(lines[li].len())];
            return (vrows.len() - 1, string_width(prefix));
        }
    }
    (0, 0)
}

fn find_vrow_at_cursor(vrows: &[VisualRow], cy: usize, cx: usize) -> &VisualRow {
    for (vi, vr) in vrows.iter().enumerate() {
        if vr.logical_line == cy && vr.start_byte <= cx && cx <= vr.end_byte {
            if cx == vr.end_byte && vr.end_byte > vr.start_byte {
                if vi + 1 < vrows.len() && vrows[vi + 1].logical_line == cy {
                    continue;
                }
            }
            return vr;
        }
    }
    vrows.last().unwrap()
}

fn byte_at_screen_pos(line: &str, seg_start: usize, seg_end: usize, screen_cx: usize) -> usize {
    let segment = &line[seg_start..seg_end];
    let mut vis_pos = 0;
    let mut byte_pos = seg_start;
    for ch in segment.chars() {
        if vis_pos >= screen_cx {
            break;
        }
        vis_pos += crate::cjk::char_width(ch);
        byte_pos += ch.len_utf8();
    }
    byte_pos.min(seg_end)
}

fn confirm_abandon(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stderr>>,
) -> io::Result<bool> {
    let msg = "放弃这篇日记？(y/n)";
    terminal.draw(|f| {
        let area = f.area();
        let mw = string_width(msg) as u16 + 4;
        let mx = (area.width.saturating_sub(mw)) / 2;
        let my = area.height / 2;
        f.render_widget(
            Paragraph::new(format!(" {}", msg)).style(
                Style::default()
                    .add_modifier(Modifier::REVERSED)
                    .add_modifier(Modifier::BOLD),
            ),
            Rect::new(area.x + mx, my, mw, 1),
        );
    })?;
    loop {
        let ev = event::read()?;
        if let Event::Key(key) = ev {
            if key.kind == KeyEventKind::Release {
                continue;
            }
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => return Ok(true),
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => return Ok(false),
                _ => {}
            }
        }
    }
}

fn render_editor(
    f: &mut Frame,
    lines: &[String],
    vrows: &[VisualRow],
    prompt_lines: &[String],
    _prompt_h: usize,
    text_h: usize,
    scroll_y: usize,
    vi_cursor: usize,
    scx_cursor: usize,
    prompt_text: &Option<String>,
    cy: usize,
    accent: &Style,
    md_enabled: bool,
    md_theme: &ThemeConfig,
) {
    let area = f.area();
    let w = area.width as usize;

    // Draw prompt area
    let mut draw_row = 0u16;
    if prompt_text.is_some() {
        draw_row += 1;
        for pl in prompt_lines {
            f.render_widget(
                Paragraph::new(format!("   {}", pl)).style(accent.add_modifier(Modifier::DIM)),
                Rect::new(area.x + 2, area.y + draw_row, area.width.saturating_sub(4), 1),
            );
            draw_row += 1;
        }
        draw_row += 1;
        let sep = "─".repeat(w.saturating_sub(2));
        f.render_widget(
            Paragraph::new(sep).style(Style::default().add_modifier(Modifier::DIM)),
            Rect::new(area.x + 1, area.y + draw_row, area.width.saturating_sub(2), 1),
        );
        draw_row += 1;
    }

    // Draw text
    for i in 0..text_h {
        let vi = scroll_y + i;
        if vi >= vrows.len() {
            break;
        }
        let vr = &vrows[vi];
        let segment = &lines[vr.logical_line][vr.start_byte..vr.end_byte];

        if md_enabled {
            let is_first = vr.start_byte == 0;
            let role = if is_first {
                detect_md_role(&lines[vr.logical_line])
            } else {
                MdRole::Continuation
            };
            let spans = highlight_inline(segment, role, md_theme);
            f.render_widget(
                Paragraph::new(Line::from(spans)),
                Rect::new(area.x, area.y + draw_row + i as u16, area.width, 1),
            );
        } else {
            f.render_widget(
                Paragraph::new(segment.to_string()),
                Rect::new(area.x, area.y + draw_row + i as u16, area.width, 1),
            );
        }
    }

    // Cursor
    let screen_row = draw_row + (vi_cursor.saturating_sub(scroll_y)) as u16;
    if screen_row < area.height.saturating_sub(2) {
        f.set_cursor_position(ratatui::layout::Position::new(
            area.x + scx_cursor as u16,
            screen_row,
        ));
    }

    // Help bar
    let mut help_parts = vec![" ^W 完成", "^Q 放弃"];
    let config = load_config();
    if !config.deepseek_api_key.is_empty() {
        help_parts.push("^P AI提示");
    }
    if !config.flomo_email.is_empty() && !config.flomo_password.is_empty() {
        help_parts.push("^S 发送Flomo");
    }
    let help = format!(" {}", help_parts.join("  "));
    f.render_widget(
        Paragraph::new(help).style(Style::default().add_modifier(Modifier::DIM)),
        Rect::new(area.x, area.y + area.height - 2, area.width, 1),
    );

    // Status bar
    let wc = word_count(lines);
    let date_display = Local::now().format("%Y年%m月%d日").to_string();
    let mode = if prompt_text.is_some() {
        "提示写作"
    } else {
        "自由写作"
    };
    let md_label = if md_enabled { " MD " } else { "" };
    let mode_label = format!("{}  ·  {}{}", date_display, mode, md_label);
    let line_info = format!("第{}行/共{}行  {}字 ", cy + 1, lines.len(), wc);
    let status = format_status_bar(&mode_label, &line_info, w);
    f.render_widget(
        Paragraph::new(status).style(Style::default().add_modifier(Modifier::REVERSED)),
        Rect::new(area.x, area.y + area.height - 1, area.width, 1),
    );
}

// ── Markdown highlighting ──

fn markdown_theme() -> ThemeConfig {
    ThemeConfig::default()
        .with_text_color(Color::Rgb(220, 220, 220))
        .with_muted_text_color(Color::Rgb(100, 100, 100))
        .with_primary_color(Color::Rgb(100, 180, 255))
        .with_secondary_color(Color::Rgb(180, 140, 255))
        .with_info_color(Color::Rgb(120, 180, 220))
        .with_accent_yellow(Color::Rgb(255, 200, 50))
        .with_border_color(Color::Rgb(80, 80, 80))
        .with_focused_border_color(Color::Rgb(140, 140, 140))
}

#[derive(Clone, Copy, PartialEq)]
enum MdRole {
    Heading1,
    Heading2,
    Heading3,
    ListItem,
    Blockquote,
    CodeFence,
    Continuation,
    Normal,
}

fn detect_md_role(line: &str) -> MdRole {
    let trimmed = line.trim_start();
    if trimmed.starts_with("```") {
        return MdRole::CodeFence;
    }
    if trimmed.starts_with("### ") {
        return MdRole::Heading3;
    }
    if trimmed.starts_with("## ") {
        return MdRole::Heading2;
    }
    if trimmed.starts_with("# ") {
        return MdRole::Heading1;
    }
    if trimmed.starts_with('>') {
        return MdRole::Blockquote;
    }
    if let Some(c) = trimmed.chars().next() {
        if (c == '-' || c == '*' || c == '+')
            && trimmed.chars().nth(1) == Some(' ')
        {
            return MdRole::ListItem;
        }
    }
    MdRole::Normal
}

fn role_base_style(role: MdRole, theme: &ThemeConfig) -> Style {
    let base = Style::default().fg(theme.get_text_color());
    match role {
        MdRole::Heading1 => base.add_modifier(Modifier::BOLD).fg(theme.get_primary_color()),
        MdRole::Heading2 => base.add_modifier(Modifier::BOLD).fg(theme.get_secondary_color()),
        MdRole::Heading3 => base.add_modifier(Modifier::BOLD).fg(theme.get_info_color()),
        MdRole::ListItem => base.fg(theme.get_text_color()),
        MdRole::Blockquote => base.add_modifier(Modifier::ITALIC).fg(theme.get_muted_text_color()),
        MdRole::CodeFence => base.fg(theme.get_accent_yellow()),
        MdRole::Continuation => base,
        MdRole::Normal => base,
    }
}

fn highlight_inline(text: &str, role: MdRole, theme: &ThemeConfig) -> Vec<Span<'static>> {
    let base = role_base_style(role, theme);
    let muted = Style::default()
        .fg(theme.get_muted_text_color())
        .add_modifier(Modifier::DIM);
    let code_style = Style::default().fg(theme.get_accent_yellow());
    let link_style = Style::default()
        .fg(theme.get_primary_color())
        .add_modifier(Modifier::UNDERLINED);

    let mut spans: Vec<Span<'static>> = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut plain_start = 0usize;

    macro_rules! flush_plain {
        ($end:expr) => {
            if plain_start < $end {
                let s: String = chars[plain_start..$end].iter().collect();
                if !s.is_empty() {
                    spans.push(Span::styled(s, base));
                }
            }
        };
    }

    while i < len {
        // *** bold italic ***
        if chars[i] == '*' && i + 2 < len && chars[i + 1] == '*' && chars[i + 2] == '*' {
            let mut end = i + 3;
            let mut found = false;
            while end + 2 < len {
                if chars[end] == '*' && chars[end + 1] == '*' && chars[end + 2] == '*' {
                    flush_plain!(i);
                    spans.push(Span::styled("***", muted));
                    let inner: String = chars[i + 3..end].iter().collect();
                    spans.push(Span::styled(
                        inner,
                        base.add_modifier(Modifier::BOLD | Modifier::ITALIC),
                    ));
                    spans.push(Span::styled("***", muted));
                    i = end + 3;
                    plain_start = i;
                    found = true;
                    break;
                }
                end += 1;
            }
            if !found {
                i += 3;
            }
            continue;
        }

        // ** bold ** or __ bold __
        if ((chars[i] == '*' && i + 1 < len && chars[i + 1] == '*')
            || (chars[i] == '_' && i + 1 < len && chars[i + 1] == '_'))
            && !(i + 2 < len && chars[i + 2] == chars[i])
        {
            let delim = chars[i];
            let marker: String = [delim, delim].iter().collect();
            let mut end = i + 2;
            let mut found = false;
            while end + 1 < len {
                if chars[end] == delim && chars[end + 1] == delim {
                    flush_plain!(i);
                    spans.push(Span::styled(marker.clone(), muted));
                    let inner: String = chars[i + 2..end].iter().collect();
                    spans.push(Span::styled(inner, base.add_modifier(Modifier::BOLD)));
                    spans.push(Span::styled(marker.clone(), muted));
                    i = end + 2;
                    plain_start = i;
                    found = true;
                    break;
                }
                end += 1;
            }
            if !found {
                i += 2;
            }
            continue;
        }

        // * italic * or _ italic _ (single, not double/triple)
        if chars[i] == '*' || chars[i] == '_' {
            // Not part of ** or ***
            if i + 1 < len && chars[i + 1] == chars[i] {
                i += 1;
                continue;
            }
            let delim = chars[i];
            let mut end = i + 1;
            let mut found = false;
            while end < len {
                if chars[end] == delim
                    && !(end + 1 < len && chars[end + 1] == delim)
                {
                    flush_plain!(i);
                    spans.push(Span::styled(
                        delim.to_string(),
                        muted,
                    ));
                    let inner: String = chars[i + 1..end].iter().collect();
                    spans.push(Span::styled(inner, base.add_modifier(Modifier::ITALIC)));
                    spans.push(Span::styled(
                        delim.to_string(),
                        muted,
                    ));
                    i = end + 1;
                    plain_start = i;
                    found = true;
                    break;
                }
                end += 1;
            }
            if !found {
                i += 1;
            }
            continue;
        }

        // ~~ strikethrough ~~
        if chars[i] == '~' && i + 1 < len && chars[i + 1] == '~' {
            let mut end = i + 2;
            let mut found = false;
            while end + 1 < len {
                if chars[end] == '~' && chars[end + 1] == '~' {
                    flush_plain!(i);
                    spans.push(Span::styled("~~", muted));
                    let inner: String = chars[i + 2..end].iter().collect();
                    spans.push(Span::styled(
                        inner,
                        base.add_modifier(Modifier::CROSSED_OUT),
                    ));
                    spans.push(Span::styled("~~", muted));
                    i = end + 2;
                    plain_start = i;
                    found = true;
                    break;
                }
                end += 1;
            }
            if !found {
                i += 2;
            }
            continue;
        }

        // ` inline code `
        if chars[i] == '`' {
            let mut end = i + 1;
            let mut found = false;
            while end < len {
                if chars[end] == '`' {
                    flush_plain!(i);
                    spans.push(Span::styled("`", muted));
                    let inner: String = chars[i + 1..end].iter().collect();
                    spans.push(Span::styled(inner, code_style));
                    spans.push(Span::styled("`", muted));
                    i = end + 1;
                    plain_start = i;
                    found = true;
                    break;
                }
                end += 1;
            }
            if !found {
                i += 1;
            }
            continue;
        }

        // [link text](url)
        if chars[i] == '[' {
            let mut bracket_end = i + 1;
            let mut found_link = false;
            while bracket_end < len {
                if chars[bracket_end] == ']'
                    && bracket_end + 1 < len
                    && chars[bracket_end + 1] == '('
                {
                    let mut paren_end = bracket_end + 2;
                    while paren_end < len {
                        if chars[paren_end] == ')' {
                            flush_plain!(i);
                            spans.push(Span::styled("[", muted));
                            let link_text: String = chars[i + 1..bracket_end].iter().collect();
                            spans.push(Span::styled(link_text, link_style));
                            spans.push(Span::styled("](", muted));
                            let url_text: String =
                                chars[bracket_end + 2..paren_end].iter().collect();
                            spans.push(Span::styled(url_text, muted));
                            spans.push(Span::styled(")", muted));
                            i = paren_end + 1;
                            plain_start = i;
                            found_link = true;
                            break;
                        }
                        paren_end += 1;
                    }
                    break;
                }
                bracket_end += 1;
            }
            if found_link {
                continue;
            }
        }

        i += 1;
    }

    flush_plain!(len);
    spans
}
