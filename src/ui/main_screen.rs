use crate::cjk::string_width;
use crate::journal;
use crate::ui::theme::{self, fill_background};
use chrono::Local;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
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
    loop {
        terminal.draw(|f| {
            draw_main(f);
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

fn day_style(is_today: bool, is_weekend: bool) -> Style {
    if is_today {
        theme::accent().add_modifier(Modifier::BOLD)
    } else if is_weekend {
        Style::default().fg(theme::PURPLE)
    } else {
        theme::text()
    }
}

fn draw_main(f: &mut Frame) {
    fill_background(f);
    let area = f.area();
    let today = Local::now().date_naive();
    let week_dates = journal::get_week_dates();
    let today_count = journal::entry_count_today();
    let streak = journal::get_streak();
    let total = journal::get_total_entries();

    // ── Title ──
    let title = "个 人 日 记";
    let tw = string_width(title) as u16;
    f.render_widget(
        Paragraph::new(title).style(theme::title_style()),
        centered_rect(area, tw, 1, 1),
    );

    // Decorative divider
    let divider = "─".repeat(36);
    let dw = string_width(&divider) as u16;
    f.render_widget(
        Paragraph::new(divider).style(theme::dimmed()),
        centered_rect(area, dw, 1, 2),
    );

    // ── Week tracker with per-day colors ──
    let day_names = ["一", "二", "三", "四", "五", "六", "日"];
    let mut day_spans: Vec<Span> = vec![Span::styled("  ", theme::text())];
    let mut mark_spans: Vec<Span> = vec![Span::styled("  ", theme::text())];

    for (i, d) in week_dates.iter().enumerate() {
        let dstr = d.format("%Y-%m-%d").to_string();
        let is_today = *d == today;
        let is_weekend = i >= 5;
        let has_entry = journal::entry_exists(&dstr);
        let name = day_names[i];
        let style = day_style(is_today, is_weekend);

        let label = if is_today {
            format!("[{}]", name)
        } else {
            format!(" {} ", name)
        };
        day_spans.push(Span::styled(label, style));
        day_spans.push(Span::styled("  ", theme::text()));

        let mark = if has_entry {
            Span::styled(" ✓  ", Style::default().fg(theme::GREEN).add_modifier(Modifier::BOLD))
        } else if *d <= today {
            Span::styled(" ·  ", theme::dimmed())
        } else {
            Span::styled("    ", theme::text())
        };
        mark_spans.push(mark);
        mark_spans.push(Span::styled("  ", theme::text()));
    }

    let day_line = Line::from(day_spans);
    let mark_line = Line::from(mark_spans);
    let day_w = day_line.width() as u16;
    let mark_w = mark_line.width() as u16;

    f.render_widget(
        Paragraph::new(day_line),
        centered_rect(area, day_w, 1, 4),
    );
    f.render_widget(
        Paragraph::new(mark_line),
        centered_rect(area, mark_w, 1, 5),
    );

    // ── Stats ──
    let stats_spans = vec![
        Span::styled(format!("连续: {} 天", streak), theme::accent().add_modifier(Modifier::BOLD)),
        Span::styled("  ·  ", theme::muted()),
        Span::styled(format!("总计: {} 篇", total), theme::text()),
    ];
    let stats_line = Line::from(stats_spans);
    let sw = stats_line.width() as u16;
    f.render_widget(
        Paragraph::new(stats_line),
        centered_rect(area, sw, 1, 7),
    );

    // ── Today status ──
    let (status_line, status_w) = if today_count > 0 {
        let s = if today_count == 1 {
            "✓ 今日已写 1 篇".to_string()
        } else {
            format!("✓ 今日已写 {} 篇", today_count)
        };
        let w = string_width(&s) as u16;
        let spans = vec![
            Span::styled("✓", Style::default().fg(theme::GREEN).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" 今日已写 {} 篇", today_count), theme::accent().add_modifier(Modifier::BOLD)),
        ];
        (Paragraph::new(Line::from(spans)), w)
    } else {
        let s = "今日尚未写日记".to_string();
        let w = string_width(&s) as u16;
        (Paragraph::new(s).style(theme::dimmed()), w)
    };
    f.render_widget(status_line, centered_rect(area, status_w, 1, 9));

    // ── Menu ──
    let menu_items: Vec<Vec<Span>> = vec![
        vec![
            Span::styled("[p]", theme::accent()),
            Span::styled(" 提示写作", theme::text()),
        ],
        vec![
            Span::styled("[f]", theme::accent()),
            Span::styled(" 自由写作", theme::text()),
        ],
        if total > 0 {
            vec![
                Span::styled("[v]", theme::accent()),
                Span::styled(" 查看过往日记", theme::text()),
            ]
        } else {
            vec![
                Span::styled("[v]", theme::muted()),
                Span::styled(" 查看过往日记", theme::dimmed()),
            ]
        },
        vec![
            Span::styled("[w]", theme::accent()),
            Span::styled(" 同步到WebDAV", theme::text()),
        ],
        vec![
            Span::styled("[s]", theme::accent()),
            Span::styled(" 设置", theme::text()),
        ],
        vec![],
        vec![
            Span::styled("[q]", theme::muted()),
            Span::styled(" 退出", theme::dimmed()),
        ],
    ];

    let mut row = 12u16;
    for item in &menu_items {
        if item.is_empty() {
            row += 1;
            continue;
        }
        let line = Line::from(item.clone());
        let w = line.width() as u16;
        f.render_widget(
            Paragraph::new(line),
            centered_rect(area, w, 1, row),
        );
        row += 1;
    }
}

fn centered_rect(area: Rect, width: u16, height: u16, y_offset: u16) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + y_offset;
    Rect::new(x, y, width, height)
}
