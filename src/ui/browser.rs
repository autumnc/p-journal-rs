use crate::cjk::string_width;
use crate::config::{load_config, strip_journal_ext};
use crate::flomo::send_to_flomo;
use crate::journal::{extract_body_from_entry, list_entries, read_entry};
use crate::ui::theme::{self, fill_background};
use chrono::NaiveDateTime;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::Rect,
    widgets::Paragraph,
    Frame,
};
use std::fs;
use std::io;

pub enum BrowserAction {
    ViewFile(String),
    EditFile(String),
    Back,
}

pub fn entry_browser(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stderr>>,
) -> io::Result<BrowserAction> {
    let mut sel: usize = 0;
    let mut scroll_off: usize = 0;
    let mut search_query: Option<String> = None;

    loop {
        let all = list_entries();
        let mut entries = match &search_query {
            Some(q) if !q.is_empty() => filter_entries(&all, q),
            _ => all,
        };

        terminal.draw(|f| {
            draw_browser(f, &entries, sel, scroll_off, search_query.as_deref());
        })?;

        let ev = event::read()?;
        let Event::Key(key) = ev else { continue };
        if key.kind == KeyEventKind::Release {
            continue;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(BrowserAction::Back),
            KeyCode::Char('/') => {
                let initial = search_query.clone().unwrap_or_default();
                if let Some(q) = input_query(terminal, "搜索: ", &initial)? {
                    let q = q.trim().to_string();
                    search_query = if q.is_empty() { None } else { Some(q) };
                }
                sel = 0;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                sel = sel.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !entries.is_empty() {
                    sel = (sel + 1).min(entries.len() - 1);
                }
            }
            KeyCode::Home => sel = 0,
            KeyCode::End => {
                if !entries.is_empty() {
                    sel = entries.len() - 1;
                }
            }
            KeyCode::Enter => {
                if let Some(fname) = entries.get(sel) {
                    return Ok(BrowserAction::ViewFile(fname.clone()));
                }
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                if let Some(fname) = entries.get(sel) {
                    return Ok(BrowserAction::EditFile(fname.clone()));
                }
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                if let Some(fname) = entries.get(sel).cloned() {
                    if confirm_delete(terminal, &fname)? {
                        let _ = fs::remove_file(crate::config::journal_dir().join(&fname));
                        show_message(terminal, "已删除", 1)?;
                        let after = list_entries();
                        let all_empty = after.is_empty();
                        entries = match &search_query {
                            Some(q) if !q.is_empty() => filter_entries(&after, q),
                            _ => after,
                        };
                        if all_empty {
                            return Ok(BrowserAction::Back);
                        }
                        sel = sel.min(entries.len().saturating_sub(1));
                    }
                }
            }
            KeyCode::Char('f') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                if let Some(fname) = entries.get(sel) {
                    if let Some(content) = read_entry(fname) {
                        let body = extract_body_from_entry(&content);
                        if !body.is_empty() {
                            let mut config = load_config();
                            let (_, msg) = send_to_flomo(&body, &mut config);
                            show_message(terminal, &msg, 2)?;
                        } else {
                            show_message(terminal, "日记内容为空", 1)?;
                        }
                    }
                }
            }
            _ => {}
        }

        let usable = (terminal.size()?.height as usize).saturating_sub(3);
        keep_selection_visible(sel, &mut scroll_off, usable, entries.len());
    }
}

fn keep_selection_visible(sel: usize, scroll_off: &mut usize, usable: usize, entry_count: usize) {
    let max_scroll = entry_count.saturating_sub(usable);
    *scroll_off = (*scroll_off).min(max_scroll);
    if usable > 0 {
        if sel < *scroll_off {
            *scroll_off = sel;
        } else if sel >= *scroll_off + usable {
            *scroll_off = sel - usable + 1;
        }
    }
    *scroll_off = (*scroll_off).min(max_scroll);
}

fn filter_entries(all: &[String], query: &str) -> Vec<String> {
    let needle = query.to_lowercase();
    all.iter()
        .filter(|f| {
            read_entry(f)
                .map(|c| c.to_lowercase().contains(&needle))
                .unwrap_or(false)
        })
        .cloned()
        .collect()
}

fn input_query(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stderr>>,
    label: &str,
    initial: &str,
) -> io::Result<Option<String>> {
    let mut value = initial.to_string();
    loop {
        terminal.draw(|f| draw_input_query(f, label, &value))?;
        let ev = event::read()?;
        let Event::Key(key) = ev else { continue };
        if key.kind == KeyEventKind::Release {
            continue;
        }
        match key.code {
            KeyCode::Esc => return Ok(None),
            KeyCode::Enter => return Ok(Some(value)),
            KeyCode::Backspace => {
                if let Some(ch) = value.chars().next_back() {
                    value.truncate(value.len() - ch.len_utf8());
                }
            }
            KeyCode::Char(c) => value.push(c),
            _ => {}
        }
    }
}

fn draw_input_query(f: &mut Frame, label: &str, value: &str) {
    fill_background(f);
    let area = f.area();
    let content = format!("{}{}", label, value);
    let mw = (string_width(&content) as u16 + 2)
        .max(20)
        .min(area.width.saturating_sub(2));
    let mh = 4u16;
    let mx = (area.width.saturating_sub(mw)) / 2;
    let my = (area.height.saturating_sub(mh)) / 2;

    for i in 0..mh {
        let pad = " ".repeat(mw as usize);
        f.render_widget(
            Paragraph::new(pad).style(theme::overlay_bg()),
            Rect::new(area.x + mx, my + i, mw, 1),
        );
    }
    let budget = mw.saturating_sub(2) as usize;
    let display = if string_width(&content) > budget {
        let mut trimmed = String::new();
        let mut w = 0usize;
        for ch in content.chars() {
            let cw = crate::cjk::char_width(ch);
            if w + cw > budget.saturating_sub(1) {
                break;
            }
            w += cw;
            trimmed.push(ch);
        }
        format!("…{}", trimmed)
    } else {
        content
    };
    f.render_widget(
        Paragraph::new(format!("  {}", display)).style(theme::overlay_text()),
        Rect::new(area.x + mx, my + 1, mw, 1),
    );
    f.render_widget(
        Paragraph::new("  [回车]确定  [Esc]取消").style(theme::help_bar()),
        Rect::new(area.x + mx, my + 2, mw, 1),
    );
}

fn draw_browser(
    f: &mut Frame,
    entries: &[String],
    sel: usize,
    scroll_off: usize,
    search_query: Option<&str>,
) {
    fill_background(f);
    let area = f.area();
    let h = area.height as usize;
    let w = area.width as usize;
    let usable = h.saturating_sub(3);

    // Header
    f.render_widget(
        Paragraph::new(" 过往日记").style(theme::title_style()),
        Rect::new(area.x, area.y, area.width, 1),
    );

    if entries.is_empty() {
        let msg = if search_query.is_some() {
            "无匹配的日记。"
        } else {
            "暂无日记。"
        };
        let mw = string_width(msg) as u16;
        f.render_widget(
            Paragraph::new(msg).style(theme::dimmed()),
            Rect::new(
                area.x + (area.width.saturating_sub(mw)) / 2,
                area.y + area.height / 2,
                mw,
                1,
            ),
        );
    } else {
        for i in 0..usable {
            let idx = scroll_off + i;
            if idx >= entries.len() {
                break;
            }
            let fname = &entries[idx];
            let date_part = strip_journal_ext(fname);
            let display_date = if let Ok(dt) =
                NaiveDateTime::parse_from_str(date_part, "%Y-%m-%d_%H%M%S")
            {
                dt.format("%Y年%m月%d日 %H:%M").to_string()
            } else {
                date_part.to_string()
            };

            let content = read_entry(fname);
            let mut preview = String::new();
            if let Some(ref c) = content {
                for line in c.lines() {
                    let stripped = line.trim();
                    if !stripped.is_empty()
                        && !stripped.starts_with("日期:")
                        && !stripped.starts_with("字数:")
                        && !stripped.starts_with("提示词:")
                        && stripped != "自由写作"
                    {
                        let remaining = w
                            .saturating_sub(string_width(&display_date))
                            .saturating_sub(10);
                        preview = stripped.chars().take(remaining).collect();
                        break;
                    }
                }
                if preview.is_empty() {
                    if let Some(line) = c.lines().find(|l| !l.trim().is_empty()) {
                        let remaining = w
                            .saturating_sub(string_width(&display_date))
                            .saturating_sub(10);
                        preview = line.trim().chars().take(remaining).collect();
                    }
                }
            }

            let row = i + 1;
            let (prefix, style) = if idx == sel {
                (" › ", theme::selected())
            } else {
                ("   ", theme::text())
            };

            let line = format!("{}{}  {}", prefix, display_date, preview);
            let line_w = string_width(&line);
            let pad = if line_w < w {
                " ".repeat(w - line_w)
            } else {
                String::new()
            };
            f.render_widget(
                Paragraph::new(format!("{}{}", line, pad)).style(style),
                Rect::new(area.x, area.y + row as u16, area.width, 1),
            );
        }
    }

    // Scrollbar
    if entries.len() > usable {
        let bar_h = ((usable * usable) / entries.len()).max(1);
        let bar_top = (scroll_off * usable) / entries.len();
        for bi in 0..bar_h {
            let by = 1u16 + bar_top as u16 + bi as u16;
            if by < area.height.saturating_sub(2) {
                f.render_widget(
                    Paragraph::new("│").style(theme::dimmed()),
                    Rect::new(area.width - 1, by, 1, 1),
                );
            }
        }
    }

    // Help bar
    let help = " [回车] 阅读  [e] 编辑  [d] 删除  [/] 搜索  [^F] 发送Flomo  [q] 返回";
    f.render_widget(
        Paragraph::new(help).style(theme::help_bar()),
        Rect::new(area.x, area.y + area.height - 2, area.width, 1),
    );

    // Status bar
    let left = match search_query {
        Some(q) if !q.is_empty() => {
            let short: String = q.chars().take(20).collect();
            format!(" ~/日记 · 搜索:{}", short)
        }
        _ => " ~/日记".to_string(),
    };
    let right = format!("{} 篇 ", entries.len());
    let status = format_status_bar(&left, &right, w);
    f.render_widget(
        Paragraph::new(status).style(theme::status_bar()),
        Rect::new(area.x, area.y + area.height - 1, area.width, 1),
    );
}

fn confirm_delete(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stderr>>,
    fname: &str,
) -> io::Result<bool> {
    let date_part = strip_journal_ext(fname);
    let display_date = if let Ok(dt) = NaiveDateTime::parse_from_str(date_part, "%Y-%m-%d_%H%M%S") {
        dt.format("%Y年%m月%d日 %H:%M").to_string()
    } else {
        date_part.to_string()
    };
    let msg = format!("删除 {} 的日记？(y/n)", display_date);

    terminal.draw(|f| {
        fill_background(f);
        let area = f.area();
        let mw = string_width(&msg) as u16 + 4;
        let mh = 3u16;
        let mx = (area.width.saturating_sub(mw)) / 2;
        let my = (area.height.saturating_sub(mh)) / 2;

        for i in 0..mh {
            let pad = " ".repeat(mw as usize);
            f.render_widget(
                Paragraph::new(pad).style(theme::overlay_bg()),
                Rect::new(area.x + mx, my + i, mw, 1),
            );
        }
        f.render_widget(
            Paragraph::new(format!("  {}", msg)).style(theme::overlay_text()),
            Rect::new(area.x + mx, my + 1, mw, 1),
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

pub fn show_message(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stderr>>,
    msg: &str,
    duration_secs: u64,
) -> io::Result<()> {
    let msg = msg.to_string();
    terminal.draw(|f| {
        fill_background(f);
        let area = f.area();
        let mw = (string_width(&msg) + 6) as u16;
        let mh = 3u16;
        let mx = (area.width.saturating_sub(mw)) / 2;
        let my = (area.height.saturating_sub(mh)) / 2;

        for i in 0..mh {
            let pad = " ".repeat(mw as usize);
            f.render_widget(
                Paragraph::new(pad).style(theme::overlay_bg()),
                Rect::new(area.x + mx, my + i, mw, 1),
            );
        }
        f.render_widget(
            Paragraph::new(format!("   {}   ", msg)).style(theme::overlay_text()),
            Rect::new(area.x + mx + 2, my + 1, mw.saturating_sub(4), 1),
        );
    })?;
    std::thread::sleep(std::time::Duration::from_secs(duration_secs));
    Ok(())
}

pub fn format_status_bar(left: &str, right: &str, width: usize) -> String {
    let lw = string_width(left);
    let rw = string_width(right);
    let pad = width.saturating_sub(lw + rw);
    format!("{}{}{}", left, " ".repeat(pad), right)
}
