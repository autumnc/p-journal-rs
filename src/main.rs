mod cjk;
mod config;
mod deepseek;
mod flomo;
mod journal;
mod prompts;
mod ui;
mod webdav;

use crate::journal::save_entry;
use crate::prompts::PROMPTS;
use crate::ui::browser::{entry_browser, show_message};
use crate::ui::editor::{journal_editor, EditorResult};
use crate::ui::main_screen::{main_screen, MainAction};
use crate::ui::settings::settings_screen;
use crate::ui::viewer::entry_viewer;
use chrono::Local;
use crossterm::{
    event::{
        KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
        PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use rand::Rng as _;
use ratatui::backend::CrosstermBackend;
use std::io;
use std::panic;

fn main() -> io::Result<()> {
    let supports_ke = matches!(
        crossterm::terminal::supports_keyboard_enhancement(),
        Ok(true)
    );

    let original_hook = panic::take_hook();
    let ke = supports_ke;
    panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal(ke);
        original_hook(info);
    }));

    enable_raw_mode()?;
    let mut stderr = io::stderr();

    if supports_ke {
        execute!(
            stderr,
            EnterAlternateScreen,
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
            ),
        )?;
    } else {
        execute!(stderr, EnterAlternateScreen)?;
    }

    let backend = CrosstermBackend::new(io::stderr());
    let mut terminal = ratatui::Terminal::new(backend)?;

    journal::ensure_journal_dir();

    loop {
        let action = main_screen(&mut terminal)?;

        match action {
            MainAction::Quit => break,
            MainAction::Settings => {
                settings_screen(&mut terminal)?;
            }
            MainAction::Webdav => {
                let config = config::load_config();
                show_message(&mut terminal, "正在双向同步 WebDAV...", 0)?;
                terminal.clear()?;
                let (_, msg) = webdav::sync_to_webdav(&config);
                show_message(&mut terminal, &msg, 3)?;
            }
            MainAction::View => loop {
                match entry_browser(&mut terminal)? {
                    crate::ui::browser::BrowserAction::ViewFile(filename) => {
                        entry_viewer(&mut terminal, &filename)?;
                    }
                    crate::ui::browser::BrowserAction::Back => break,
                }
            },
            MainAction::Prompt | MainAction::Freewrite => {
                let prompt_text = match action {
                    MainAction::Prompt => {
                        let mut rng = rand::thread_rng();
                        let idx = rng.gen_range(0..PROMPTS.len());
                        Some(PROMPTS[idx].to_string())
                    }
                    _ => None,
                };

                let (text, active_prompt) = match journal_editor(&mut terminal, prompt_text.clone())? {
                    EditorResult::Commit(text, prompt) => (text, prompt),
                    EditorResult::Cancel => continue,
                };

                if text.is_empty() {
                    continue;
                }

                let full_text = if let Some(ref pt) = active_prompt {
                    format!("提示词: {}\n\n{}", pt, text)
                } else {
                    format!("自由写作\n\n{}", text)
                };

                let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                let wc = crate::cjk::word_count(&[text.clone()]);
                let header = format!("日期: {}\n字数: {}\n\n", timestamp, wc);
                let full_text = format!("{}{}", header, full_text);

                save_entry(&full_text).ok();
            }
        }
    }

    restore_terminal(supports_ke)?;
    println!("再见。");
    Ok(())
}

fn restore_terminal(supports_ke: bool) -> io::Result<()> {
    let mut stderr = io::stderr();
    if supports_ke {
        execute!(stderr, PopKeyboardEnhancementFlags)?;
    }
    execute!(stderr, LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
