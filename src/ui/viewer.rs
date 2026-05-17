use crate::config::{file_ext, load_config};
use crate::flomo::send_to_flomo;
use crate::journal::{extract_body_from_entry, read_entry};
use crate::ui::browser::{format_status_bar, show_message};
use chrono::NaiveDateTime;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Paragraph,
};
use std::io;

pub fn entry_viewer(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stderr>>,
    filename: &str,
) -> io::Result<()> {
    let content = match read_entry(filename) {
        Some(c) => c,
        None => return Ok(()),
    };

    let date_part = filename.trim_end_matches(file_ext());
    let display_date = if let Ok(dt) = NaiveDateTime::parse_from_str(date_part, "%Y-%m-%d_%H%M%S") {
        dt.format("%Y年%m月%d日 %H:%M").to_string()
    } else {
        date_part.to_string()
    };

    // Build display lines with styles
    struct DisplayLine {
        text: String,
        style: Style,
    }

    let accent = Style::default().fg(Color::Yellow);
    let mut lines: Vec<DisplayLine> = Vec::new();
    lines.push(DisplayLine {
        text: String::new(),
        style: Style::default(),
    });
    lines.push(DisplayLine {
        text: format!("  {}", display_date),
        style: accent.add_modifier(Modifier::BOLD),
    });
    lines.push(DisplayLine {
        text: String::new(),
        style: Style::default(),
    });

    let term_w = terminal.size()?.width as usize;
    let wrap_width = term_w.saturating_sub(4);

    for raw_line in content.lines() {
        let stripped = raw_line.trim();
        if stripped.starts_with("日期:") || stripped.starts_with("字数:") {
            continue;
        } else if let Some(prompt) = stripped.strip_prefix("提示词:") {
            let prompt_text = prompt.trim();
            let wrapped = textwrap::fill(prompt_text, wrap_width);
            for wl in wrapped.lines() {
                lines.push(DisplayLine {
                    text: format!("  {}", wl),
                    style: accent.add_modifier(Modifier::DIM),
                });
            }
            lines.push(DisplayLine {
                text: String::new(),
                style: Style::default(),
            });
        } else if stripped == "自由写作" {
            lines.push(DisplayLine {
                text: "  自由写作".to_string(),
                style: Style::default().add_modifier(Modifier::DIM),
            });
            lines.push(DisplayLine {
                text: String::new(),
                style: Style::default(),
            });
        } else if stripped.is_empty() {
            lines.push(DisplayLine {
                text: String::new(),
                style: Style::default(),
            });
        } else {
            let wrapped = textwrap::fill(raw_line, wrap_width);
            for wl in wrapped.lines() {
                lines.push(DisplayLine {
                    text: format!("  {}", wl),
                    style: Style::default(),
                });
            }
        }
    }
    lines.push(DisplayLine {
        text: String::new(),
        style: Style::default(),
    });

    let mut scroll: usize = 0;

    loop {
        terminal.draw(|f| {
            let area = f.area();
            let h = area.height as usize;
            let text_h = h.saturating_sub(2);
            let max_scroll = lines.len().saturating_sub(text_h);
            let scroll = scroll.min(max_scroll);

            for i in 0..text_h {
                let line_idx = scroll + i;
                if line_idx >= lines.len() {
                    break;
                }
                let ld = &lines[line_idx];
                f.render_widget(
                    Paragraph::new(ld.text.as_str()).style(ld.style),
                    Rect::new(area.x, area.y + i as u16, area.width, 1),
                );
            }

            // Status bar
            let pos = if lines.len() > text_h {
                let pct = if max_scroll > 0 {
                    (scroll * 100) / max_scroll
                } else {
                    100
                };
                format!(" {}%", pct)
            } else {
                " 100%".to_string()
            };
            let left = format!(" {}  (只读)", display_date);
            let status = format_status_bar(&left, &pos, area.width as usize);
            f.render_widget(
                Paragraph::new(status).style(Style::default().add_modifier(Modifier::REVERSED)),
                Rect::new(area.x, area.y + area.height - 1, area.width, 1),
            );

            // Help bar
            let help = " ↑↓ 滚动  g/G 顶/底  ^S 发送Flomo  q:返回";
            f.render_widget(
                Paragraph::new(help).style(Style::default().add_modifier(Modifier::DIM)),
                Rect::new(area.x, area.y + area.height - 2, area.width, 1),
            );
        })?;

        let ev = event::read()?;
        let Event::Key(key) = ev else { continue };
        if key.kind == KeyEventKind::Release {
            continue;
        }

        let text_h = (terminal.size()?.height as usize).saturating_sub(2);
        let max_scroll = lines.len().saturating_sub(text_h);

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
            KeyCode::Up | KeyCode::Char('k') => scroll = scroll.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => scroll = (scroll + 1).min(max_scroll),
            KeyCode::PageUp | KeyCode::Char(' ') => scroll = scroll.saturating_sub(text_h),
            KeyCode::PageDown => scroll = (scroll + text_h).min(max_scroll),
            KeyCode::Char('g') => scroll = 0,
            KeyCode::Char('G') => scroll = max_scroll,
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let body = extract_body_from_entry(&content);
                if !body.is_empty() {
                    let mut config = load_config();
                    let (_, msg) = send_to_flomo(&body, &mut config);
                    show_message(terminal, &msg, 2)?;
                } else {
                    show_message(terminal, "日记内容为空", 1)?;
                }
            }
            _ => {}
        }
    }
}
