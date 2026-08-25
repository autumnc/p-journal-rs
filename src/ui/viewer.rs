use crate::config::{load_config, strip_journal_ext};
use crate::flomo::send_to_flomo;
use crate::journal::{extract_body_from_entry, read_entry, update_entry};
use crate::ui::browser::{format_status_bar, show_message};
use crate::ui::editor::{
    detect_md_role, highlight_inline, journal_editor_with_initial, markdown_theme, EditorResult,
    MdRole,
};
use crate::ui::theme::{self, fill_background};
use chrono::NaiveDateTime;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};
use std::io;

pub fn entry_viewer(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stderr>>,
    filename: &str,
) -> io::Result<()> {
    'reload: loop {
        let content = match read_entry(filename) {
            Some(c) => c,
            None => return Ok(()),
        };

        let md_enabled = load_config().markdown_enabled;
        let md_theme = markdown_theme();

        let date_part = strip_journal_ext(filename);
        let display_date = if let Ok(dt) = NaiveDateTime::parse_from_str(date_part, "%Y-%m-%d_%H%M%S") {
            dt.format("%Y年%m月%d日 %H:%M").to_string()
        } else {
            date_part.to_string()
        };

        struct DisplayLine {
            line: Line<'static>,
        }

        let mut lines: Vec<DisplayLine> = Vec::new();
        lines.push(DisplayLine {
            line: Line::from(Span::raw("")),
        });
        lines.push(DisplayLine {
            line: Line::from(Span::styled(
                format!("  {}", display_date),
                theme::accent().add_modifier(ratatui::style::Modifier::BOLD),
            )),
        });
        lines.push(DisplayLine {
            line: Line::from(Span::raw("")),
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
                        line: Line::from(Span::styled(
                            format!("  {}", wl),
                            theme::dimmed(),
                        )),
                    });
                }
                lines.push(DisplayLine {
                    line: Line::from(Span::raw("")),
                });
            } else if stripped == "自由写作" {
                lines.push(DisplayLine {
                    line: Line::from(Span::styled("  自由写作".to_string(), theme::dimmed())),
                });
                lines.push(DisplayLine {
                    line: Line::from(Span::raw("")),
                });
            } else if stripped.is_empty() {
                lines.push(DisplayLine {
                    line: Line::from(Span::raw("")),
                });
            } else if md_enabled {
                let role = detect_md_role(raw_line);
                let wrapped = textwrap::fill(raw_line, wrap_width);
                for (i, wl) in wrapped.lines().enumerate() {
                    let spans = if i == 0 {
                        highlight_inline(wl, role, &md_theme)
                    } else {
                        highlight_inline(wl, MdRole::Continuation, &md_theme)
                    };
                    let padded = pad_spans(spans, term_w);
                    lines.push(DisplayLine {
                        line: Line::from(padded),
                    });
                }
            } else {
                let wrapped = textwrap::fill(raw_line, wrap_width);
                for wl in wrapped.lines() {
                    lines.push(DisplayLine {
                        line: Line::from(Span::styled(
                            format!("  {}", wl),
                            theme::text(),
                        )),
                    });
                }
            }
        }
        lines.push(DisplayLine {
            line: Line::from(Span::raw("")),
        });

        let mut scroll: usize = 0;

        loop {
            terminal.draw(|f| {
                fill_background(f);
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
                        Paragraph::new(ld.line.clone()),
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
                    Paragraph::new(status).style(theme::status_bar()),
                    Rect::new(area.x, area.y + area.height - 1, area.width, 1),
                );

                // Help bar
                let help = " ↑↓ 滚动  g/G 顶/底  e:编辑  ^F 发送Flomo  q:返回";
                f.render_widget(
                    Paragraph::new(help).style(theme::help_bar()),
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
                KeyCode::Char('e') | KeyCode::Char('E') => {
                    if edit_loaded_entry(terminal, filename, &content)? {
                        continue 'reload;
                    }
                }
                KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
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
}

pub fn edit_entry(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stderr>>,
    filename: &str,
) -> io::Result<bool> {
    let content = match read_entry(filename) {
        Some(c) => c,
        None => return Ok(false),
    };
    edit_loaded_entry(terminal, filename, &content)
}

fn edit_loaded_entry(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stderr>>,
    filename: &str,
    content: &str,
) -> io::Result<bool> {
    let (date, prompt, body) = parse_entry_for_edit(filename, content);
    match journal_editor_with_initial(terminal, prompt.clone(), body)? {
        EditorResult::Commit(text, active_prompt) => {
            let wc = crate::cjk::word_count(&[text.clone()]);
            let full_text = if let Some(ref pt) = active_prompt {
                format!("日期: {}\n字数: {}\n\n提示词: {}\n\n{}", date, wc, pt, text)
            } else {
                format!("日期: {}\n字数: {}\n\n自由写作\n\n{}", date, wc, text)
            };
            update_entry(filename, &full_text)?;
            show_message(terminal, "已保存修改", 1)?;
            Ok(true)
        }
        EditorResult::Cancel => Ok(false),
    }
}

fn parse_entry_for_edit(filename: &str, content: &str) -> (String, Option<String>, String) {
    let mut date = content
        .lines()
        .find_map(|line| line.trim().strip_prefix("日期:").map(|d| d.trim().to_string()))
        .unwrap_or_else(|| strip_journal_ext(filename).to_string());
    if date.is_empty() {
        date = strip_journal_ext(filename).to_string();
    }

    let prompt = content
        .lines()
        .find_map(|line| line.trim().strip_prefix("提示词:").map(|p| p.trim().to_string()));
    let body = extract_body_from_entry(content);
    (date, prompt, body)
}

fn pad_spans(spans: Vec<Span<'static>>, width: usize) -> Vec<Span<'static>> {
    use crate::cjk::string_width;
    let total: usize = spans.iter().map(|s| string_width(&s.content)).sum();
    let mut result = spans;
    if total < width {
        result.push(Span::styled(" ".repeat(width - total), theme::text()));
    }
    result
}
