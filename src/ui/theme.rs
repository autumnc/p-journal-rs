use ratatui::{
    style::{Color, Modifier, Style},
    widgets::Block,
    Frame,
};

// ── Color palette ──
// NOTE: fbterm does not support 24-bit RGB on per-span backgrounds, nor the DIM
// modifier. Use only ANSI-compatible colors for span backgrounds and never use DIM.

pub const FG: Color = Color::Rgb(212, 212, 212);          // #d4d4d4
pub const ACCENT: Color = Color::Rgb(229, 192, 123);      // #e5c07b warm gold
#[allow(dead_code)]
pub const ACCENT_DIM: Color = Color::Rgb(180, 150, 90);   // dimmer gold
pub const BLUE: Color = Color::Rgb(97, 175, 239);         // #61afef
pub const PURPLE: Color = Color::Rgb(198, 120, 221);      // #c678dd
pub const TEAL: Color = Color::Rgb(86, 182, 194);         // #56b6c2
pub const MUTED: Color = Color::Rgb(92, 99, 112);         // #5c6370
pub const DIMMED_FG: Color = Color::Rgb(70, 73, 80);     // dimmed text (no DIM modifier)
pub const SURFACE: Color = Color::Rgb(30, 30, 30);        // #1e1e1e
pub const SURFACE_LIGHT: Color = Color::Rgb(44, 44, 44);  // #2c2c2c
pub const BORDER: Color = Color::Rgb(60, 60, 60);         // #3c3c3c
#[allow(dead_code)]
pub const RED: Color = Color::Rgb(224, 108, 117);         // #e06c75
#[allow(dead_code)]
pub const GREEN: Color = Color::Rgb(152, 195, 121);       // #98c379

// ── Style presets ──

/// Fill uses terminal default bg — per-span RGB bg produces garbage
/// in fbterm (`\x1b[48;2;R;G;Bm`), so avoid setting bg on spans.
pub fn text() -> Style {
    Style::default().fg(FG)
}

pub fn accent() -> Style {
    Style::default().fg(ACCENT)
}

pub fn title_style() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

pub fn muted() -> Style {
    Style::default().fg(MUTED)
}

/// Use darker fg instead of DIM modifier — fbterm does not support DIM.
pub fn dimmed() -> Style {
    Style::default().fg(DIMMED_FG)
}

pub fn status_bar() -> Style {
    Style::default().fg(FG).bg(SURFACE_LIGHT)
}

pub fn selected() -> Style {
    Style::default().fg(FG).bg(SURFACE_LIGHT)
}

pub fn help_bar() -> Style {
    Style::default().fg(MUTED)
}

pub fn overlay_bg() -> Style {
    Style::default().fg(FG).bg(SURFACE)
}

pub fn overlay_text() -> Style {
    Style::default()
        .fg(ACCENT)
        .bg(SURFACE)
        .add_modifier(Modifier::BOLD)
}

/// Bold accent, no bg — fbterm-safe (no per-span RGB background).
pub fn highlight() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

/// Fill the entire frame area with the background color.
/// Must be called first in every draw function to prevent IME ghosting / image retention.
pub fn fill_background(f: &mut Frame) {
    let area = f.area();
    f.render_widget(
        Block::new().style(Style::default().bg(Color::Reset)),
        area,
    );
}
