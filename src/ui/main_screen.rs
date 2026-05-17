use crate::cjk::string_width;
use crate::journal;
use chrono::Local;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Paragraph,
    Frame,
};
use std::io;

pub enum MainAction {
    Quit,
    Prompt,
    Freewrite,
    View,
    Webdav,
    Settings,
}

pub fn main_screen(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stderr>>,
) -> io::Result<MainAction> {
    let accent = Style::default().fg(Color::Yellow);

    loop {
        terminal.draw(|f| {
            draw_main(f, &accent);
        })?;

        let total = journal::get_total_entries();
        loop {
            let ev = event::read()?;
            let Event::Key(key) = ev else { continue };
            if key.kind == KeyEventKind::Release {
                continue;
            }
            return Ok(match key.code {
                KeyCode::Char('q') => MainAction::Quit,
                KeyCode::Char('p') => MainAction::Prompt,
                KeyCode::Char('f') => MainAction::Freewrite,
                KeyCode::Char('v') if total > 0 => MainAction::View,
                KeyCode::Char('w') => MainAction::Webdav,
                KeyCode::Char('s') => MainAction::Settings,
                _ => continue,
            });
        }
    }
}

fn draw_main(f: &mut Frame, accent: &Style) {
    let area = f.area();
    let today = Local::now().date_naive();
    let week_dates = journal::get_week_dates();
    let today_count = journal::entry_count_today();
    let streak = journal::get_streak();
    let total = journal::get_total_entries();

    // Title
    let title = "个人日记";
    let tw = string_width(title) as u16;
    let title_area = centered_rect(area, tw, 1, 1);
    f.render_widget(
        Paragraph::new(title).style(Style::default().add_modifier(Modifier::BOLD)),
        title_area,
    );

    // Week tracker
    let day_names = ["一", "二", "三", "四", "五", "六", "日"];
    let mut header_parts = Vec::new();
    let mut mark_parts = Vec::new();
    for (i, d) in week_dates.iter().enumerate() {
        let dstr = d.format("%Y-%m-%d").to_string();
        let is_today = *d == today;
        let has_entry = journal::entry_exists(&dstr);
        let name = day_names[i];
        header_parts.push(if is_today {
            format!("[{}]", name)
        } else {
            format!(" {} ", name)
        });
        if has_entry {
            mark_parts.push(" ✓  ".to_string());
        } else if *d <= today {
            mark_parts.push(" ·  ".to_string());
        } else {
            mark_parts.push("    ".to_string());
        }
    }

    let header = format!("  {}", header_parts.join("  "));
    let marks = format!("  {}", mark_parts.join("  "));
    let hw = string_width(&header) as u16;
    let mw = string_width(&marks) as u16;

    f.render_widget(Paragraph::new(header), centered_rect(area, hw, 1, 4));
    f.render_widget(
        Paragraph::new(marks).style(Style::default().add_modifier(Modifier::BOLD)),
        centered_rect(area, mw, 1, 5),
    );

    // Stats
    let stats = format!("连续: {} 天  ·  总计: {} 篇", streak, total);
    let sw = string_width(&stats) as u16;
    f.render_widget(
        Paragraph::new(stats).style(Style::default().add_modifier(Modifier::DIM)),
        centered_rect(area, sw, 1, 8),
    );

    // Today status
    let (status, status_style) = if today_count > 0 {
        let s = if today_count == 1 {
            "✓ 今日已写 1 篇".to_string()
        } else {
            format!("✓ 今日已写 {} 篇", today_count)
        };
        (s, accent.add_modifier(Modifier::BOLD))
    } else {
        ("今日尚未写日记".to_string(), Style::default().add_modifier(Modifier::DIM))
    };
    let stw = string_width(&status) as u16;
    f.render_widget(Paragraph::new(status).style(status_style), centered_rect(area, stw, 1, 10));

    // Menu
    let menu = vec![
        ("[p] 提示写作", Style::default()),
        ("[f] 自由写作", Style::default()),
        ("[v] 查看过往日记", if total > 0 { Style::default() } else { continue_style() }),
        ("[w] 同步到WebDAV", Style::default()),
        ("[s] 设置", Style::default()),
        ("", Style::default()),
        ("[q] 退出", Style::default().add_modifier(Modifier::DIM)),
    ];

    let mut row = 13u16;
    for (text, style) in &menu {
        if text.is_empty() {
            row += 1;
            continue;
        }
        let tw = string_width(text) as u16;
        f.render_widget(Paragraph::new(*text).style(*style), centered_rect(area, tw, 1, row));
        row += 1;
    }
}

fn continue_style() -> Style {
    Style::default()
}

fn centered_rect(area: Rect, width: u16, height: u16, y_offset: u16) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + y_offset;
    Rect::new(x, y, width, height)
}

