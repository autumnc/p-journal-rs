use crate::cjk::string_width;
use crate::config::{load_config, save_config, Config};
use crate::ui::theme::{self, fill_background};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::Rect,
    style::Modifier,
    widgets::Paragraph,
    Frame,
};
use std::io;

#[derive(Clone, Copy, PartialEq)]
enum FieldKind {
    Text,
    Bool,
    Choice,
}

struct FieldDef {
    key: &'static str,
    label: &'static str,
    masked: bool,
    kind: FieldKind,
}

struct GroupDef {
    title: &'static str,
    fields: &'static [FieldDef],
}

static GROUPS: &[GroupDef] = &[
    GroupDef {
        title: "── 显示 ──",
        fields: &[
            FieldDef {
                key: "markdown_enabled",
                label: "Markdown 语法高亮",
                masked: false,
                kind: FieldKind::Bool,
            },
            FieldDef {
                key: "file_format",
                label: "文件格式",
                masked: false,
                kind: FieldKind::Choice,
            },
        ],
    },
    GroupDef {
        title: "── AI ──",
        fields: &[FieldDef {
            key: "deepseek_api_key",
            label: "Deepseek API Key",
            masked: false,
            kind: FieldKind::Text,
        }],
    },
    GroupDef {
        title: "── Flomo ──",
        fields: &[
            FieldDef {
                key: "flomo_email",
                label: "邮箱",
                masked: false,
                kind: FieldKind::Text,
            },
            FieldDef {
                key: "flomo_password",
                label: "密码",
                masked: true,
                kind: FieldKind::Text,
            },
        ],
    },
    GroupDef {
        title: "── WebDAV ──",
        fields: &[
            FieldDef {
                key: "webdav_url",
                label: "服务器地址",
                masked: false,
                kind: FieldKind::Text,
            },
            FieldDef {
                key: "webdav_username",
                label: "用户名",
                masked: false,
                kind: FieldKind::Text,
            },
            FieldDef {
                key: "webdav_password",
                label: "密码",
                masked: true,
                kind: FieldKind::Text,
            },
        ],
    },
    GroupDef {
        title: "── 个人 ──",
        fields: &[
            FieldDef {
                key: "personal_experience",
                label: "经历",
                masked: false,
                kind: FieldKind::Text,
            },
            FieldDef {
                key: "personal_hobbies",
                label: "爱好",
                masked: false,
                kind: FieldKind::Text,
            },
            FieldDef {
                key: "personal_recent_status",
                label: "最近状态",
                masked: false,
                kind: FieldKind::Text,
            },
        ],
    },
];

struct FieldRow {
    group_idx: usize,
    field_idx: usize,
    is_group: bool,
}

pub fn settings_screen(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stderr>>,
) -> io::Result<()> {
    let mut config = load_config();

    // Build flat display rows
    let mut rows: Vec<FieldRow> = Vec::new();
    for (gidx, group) in GROUPS.iter().enumerate() {
        rows.push(FieldRow {
            group_idx: gidx,
            field_idx: 0,
            is_group: true,
        });
        for fidx in 0..group.fields.len() {
            rows.push(FieldRow {
                group_idx: gidx,
                field_idx: fidx,
                is_group: false,
            });
        }
    }

    let mut sel_idx: usize = 0;
    let scroll_off: usize = 0;
    let mut editing: bool = false;
    let mut edit_buf = String::new();
    let mut cursor_pos: usize = 0;

    loop {
        terminal.draw(|f| {
            draw_settings(
                f,
                &rows,
                &config,
                sel_idx,
                scroll_off,
                editing,
                &edit_buf,
                cursor_pos,
            );
        })?;

        let ev = event::read()?;
        let Event::Key(key) = ev else { continue };
        if key.kind == KeyEventKind::Release {
            continue;
        }

        if editing {
            match key.code {
                KeyCode::Esc => {
                    editing = false;
                }
                KeyCode::Enter => {
                    commit_edit(&rows[sel_idx], &edit_buf, &mut config);
                    edit_buf.clear();
                    cursor_pos = 0;
                    editing = false;
                }
                KeyCode::Backspace => {
                    if cursor_pos > 0 {
                        let prev = edit_buf
                            .char_indices()
                            .rev()
                            .find(|(i, _)| *i < cursor_pos)
                            .map(|(i, _)| i)
                            .unwrap_or(0);
                        edit_buf.remove(prev);
                        cursor_pos = prev;
                    }
                }
                KeyCode::Left => {
                    if cursor_pos > 0 {
                        cursor_pos = edit_buf
                            .char_indices()
                            .rev()
                            .find(|(i, _)| *i < cursor_pos)
                            .map(|(i, _)| i)
                            .unwrap_or(0);
                    }
                }
                KeyCode::Right => {
                    if cursor_pos < edit_buf.len() {
                        cursor_pos = edit_buf
                            .char_indices()
                            .find(|(i, _)| *i > cursor_pos)
                            .map(|(i, _)| i)
                            .unwrap_or(edit_buf.len());
                    }
                }
                KeyCode::Home => cursor_pos = 0,
                KeyCode::End => cursor_pos = edit_buf.len(),
                KeyCode::Char(c) => {
                    edit_buf.insert(cursor_pos, c);
                    cursor_pos += c.len_utf8();
                }
                _ => {}
            }
            continue;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
            KeyCode::Up | KeyCode::Char('k') => {
                if sel_idx > 0 {
                    sel_idx -= 1;
                    while sel_idx > 0 && rows[sel_idx].is_group {
                        sel_idx -= 1;
                    }
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if sel_idx + 1 < rows.len() {
                    sel_idx += 1;
                    while sel_idx < rows.len() && rows[sel_idx].is_group {
                        sel_idx += 1;
                    }
                }
                if sel_idx >= rows.len() {
                    sel_idx = rows.len().saturating_sub(1);
                }
                if rows[sel_idx].is_group && sel_idx + 1 < rows.len() {
                    sel_idx += 1;
                }
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                if !rows.is_empty() && !rows[sel_idx].is_group {
                    clear_field(&rows[sel_idx], &mut config);
                }
            }
            KeyCode::Enter => {
                if !rows.is_empty() && !rows[sel_idx].is_group {
                    let field = field_def(&rows[sel_idx]);
                    if field.kind == FieldKind::Bool {
                        toggle_bool_field(&rows[sel_idx], &mut config);
                    } else if field.kind == FieldKind::Choice {
                        toggle_choice_field(&rows[sel_idx], &mut config);
                    } else {
                        edit_buf = get_config_value(&config, field_key(&rows[sel_idx]));
                        cursor_pos = edit_buf.len();
                        editing = true;
                    }
                }
            }
            _ => {}
        }
    }
}

fn field_key(row: &FieldRow) -> &str {
    GROUPS[row.group_idx].fields[row.field_idx].key
}

fn field_def(row: &FieldRow) -> &FieldDef {
    &GROUPS[row.group_idx].fields[row.field_idx]
}

fn toggle_bool_field(row: &FieldRow, config: &mut Config) {
    let key = field_key(row);
    if key == "markdown_enabled" {
        config.markdown_enabled = !config.markdown_enabled;
    }
    save_config(config).ok();
}

fn toggle_choice_field(row: &FieldRow, config: &mut Config) {
    let key = field_key(row);
    if key == "file_format" {
        config.file_format = if config.file_format == "md" {
            "txt".to_string()
        } else {
            "md".to_string()
        };
    }
    save_config(config).ok();
}

fn get_config_value(config: &Config, key: &str) -> String {
    match key {
        "deepseek_api_key" => config.deepseek_api_key.clone(),
        "flomo_email" => config.flomo_email.clone(),
        "flomo_password" => config.flomo_password.clone(),
        "webdav_url" => config.webdav_url.clone(),
        "webdav_username" => config.webdav_username.clone(),
        "webdav_password" => config.webdav_password.clone(),
        "personal_experience" => config.personal_experience.clone(),
        "personal_hobbies" => config.personal_hobbies.clone(),
        "personal_recent_status" => config.personal_recent_status.clone(),
        "file_format" => config.file_format.clone(),
        _ => String::new(),
    }
}

fn get_config_bool(config: &Config, key: &str) -> bool {
    match key {
        "markdown_enabled" => config.markdown_enabled,
        _ => false,
    }
}

fn clear_field(row: &FieldRow, config: &mut Config) {
    let key = field_key(row);
    match key {
        "deepseek_api_key" => config.deepseek_api_key.clear(),
        "flomo_email" => config.flomo_email.clear(),
        "flomo_password" => config.flomo_password.clear(),
        "webdav_url" => config.webdav_url.clear(),
        "webdav_username" => config.webdav_username.clear(),
        "webdav_password" => config.webdav_password.clear(),
        "personal_experience" => config.personal_experience.clear(),
        "personal_hobbies" => config.personal_hobbies.clear(),
        "personal_recent_status" => config.personal_recent_status.clear(),
        "file_format" => config.file_format = "txt".to_string(),
        _ => {}
    }
    save_config(config).ok();
}

fn commit_edit(row: &FieldRow, value: &str, config: &mut Config) {
    let key = field_key(row);
    match key {
        "deepseek_api_key" => config.deepseek_api_key = value.to_string(),
        "flomo_email" => config.flomo_email = value.to_string(),
        "flomo_password" => config.flomo_password = value.to_string(),
        "webdav_url" => config.webdav_url = value.to_string(),
        "webdav_username" => config.webdav_username = value.to_string(),
        "webdav_password" => config.webdav_password = value.to_string(),
        "personal_experience" => config.personal_experience = value.to_string(),
        "personal_hobbies" => config.personal_hobbies = value.to_string(),
        "personal_recent_status" => config.personal_recent_status = value.to_string(),
        _ => {}
    }
    // Clear Flomo token if credentials changed
    if key == "flomo_email" || key == "flomo_password" {
        config.flomo_token.clear();
    }
    save_config(config).ok();
}

fn draw_settings(
    f: &mut Frame,
    rows: &[FieldRow],
    config: &Config,
    sel_idx: usize,
    scroll_off: usize,
    editing: bool,
    edit_buf: &str,
    cursor_pos: usize,
) {
    fill_background(f);
    let area = f.area();
    let h = area.height as usize;
    let w = area.width as usize;
    let usable = h.saturating_sub(3);

    // Title
    let title = "── 设置 ──";
    let title_w = string_width(title) as u16;
    let title_x = (area.width.saturating_sub(title_w)) / 2;
    f.render_widget(
        Paragraph::new(title).style(theme::title_style()),
        Rect::new(area.x + title_x, area.y, title_w, 1),
    );

    // Draw rows
    let mut draw_row = 1u16;
    for vi in scroll_off..rows.len().min(scroll_off + usable) {
        let field_row = &rows[vi];
        if field_row.is_group {
            let group = &GROUPS[field_row.group_idx];
            f.render_widget(
                Paragraph::new(format!(" {}", group.title))
                    .style(theme::accent().add_modifier(Modifier::BOLD)),
                Rect::new(area.x, area.y + draw_row, area.width, 1),
            );
        } else {
            let group = &GROUPS[field_row.group_idx];
            let field = &group.fields[field_row.field_idx];
            let value = get_config_value(config, field.key);
            let is_sel = vi == sel_idx;

            let display_value = if is_sel && editing {
                let masked_display = if field.masked && !edit_buf.is_empty() {
                    "*".repeat(edit_buf.chars().count())
                } else {
                    edit_buf.to_string()
                };
                format!("{}{}", masked_display, cursor_pos as u8)
            } else if field.kind == FieldKind::Bool {
                let val = get_config_bool(config, field.key);
                if val {
                    "✓ 开".to_string()
                } else {
                    "✗ 关".to_string()
                }
            } else if field.kind == FieldKind::Choice {
                value.to_string()
            } else if field.masked && !value.is_empty() {
                "*".repeat(value.chars().count().min(20))
            } else if !value.is_empty() {
                let char_count = value.chars().count();
                if char_count > 40 {
                    let truncated: String = value.chars().take(40).collect();
                    format!("{}...", truncated)
                } else {
                    value.to_string()
                }
            } else {
                "(未设置)".to_string()
            };

            let style = if is_sel {
                theme::selected()
            } else {
                theme::text()
            };

            let line = format!("  {}: ", field.label);
            let full_line = format!("{}{}", line, display_value);
            let full_w = string_width(&full_line);
            let padded = if full_w < w {
                format!("{}{}", full_line, " ".repeat(w - full_w))
            } else {
                full_line
            };
            f.render_widget(
                Paragraph::new(padded).style(style),
                Rect::new(area.x, area.y + draw_row, area.width, 1),
            );
        }
        draw_row += 1;
    }

    // Scrollbar
    if rows.len() > usable {
        let bar_h = ((usable * usable) / rows.len()).max(1);
        let bar_top = (scroll_off * usable) / rows.len();
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
    let help = if editing {
        " [回车] 确认  [Esc] 取消"
    } else if !rows.is_empty() && !rows[sel_idx].is_group {
        let kind = field_def(&rows[sel_idx]).kind;
        if kind == FieldKind::Bool || kind == FieldKind::Choice {
            " [回车] 切换  [q] 返回"
        } else {
            " [回车] 编辑  [d] 清空  [q] 返回"
        }
    } else {
        " [回车] 编辑  [d] 清空  [q] 返回"
    };
    f.render_widget(
        Paragraph::new(help).style(theme::help_bar()),
        Rect::new(area.x, area.y + area.height - 2, area.width, 1),
    );

    // Status bar
    let status = format_status_bar(" ~/设置", "", w);
    f.render_widget(
        Paragraph::new(status).style(theme::status_bar()),
        Rect::new(area.x, area.y + area.height - 1, area.width, 1),
    );
}

fn format_status_bar(left: &str, right: &str, width: usize) -> String {
    let lw = string_width(left);
    let rw = string_width(right);
    let pad = width.saturating_sub(lw + rw);
    format!("{}{}{}", left, " ".repeat(pad), right)
}
