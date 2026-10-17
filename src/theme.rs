//! Five named palettes applied to the finished terminal buffer.
//!
//! Only semantic interface colors are remapped; animation artwork keeps its colors.

use crate::{AMBER, BG, CREAM, GREEN};
use ratatui::{buffer::Buffer, style::Color};
pub const NAMES: [&str; 5] = ["amber", "forest", "ocean", "rose", "mono"];
const PALETTES: [[Color; 5]; 5] = [
    [BG, CREAM, AMBER, GREEN, Color::Rgb(107, 101, 86)],
    [
        Color::Rgb(24, 34, 29),
        Color::Rgb(220, 231, 213),
        Color::Rgb(160, 194, 133),
        Color::Rgb(118, 177, 148),
        Color::Rgb(82, 109, 89),
    ],
    [
        Color::Rgb(23, 31, 43),
        Color::Rgb(215, 228, 237),
        Color::Rgb(128, 189, 211),
        Color::Rgb(139, 174, 216),
        Color::Rgb(76, 101, 125),
    ],
    [
        Color::Rgb(36, 27, 34),
        Color::Rgb(237, 218, 226),
        Color::Rgb(214, 157, 180),
        Color::Rgb(182, 165, 213),
        Color::Rgb(112, 82, 101),
    ],
    [
        Color::Rgb(26, 26, 26),
        Color::Rgb(225, 225, 225),
        Color::Rgb(250, 250, 250),
        Color::Rgb(179, 179, 179),
        Color::Rgb(100, 100, 100),
    ],
];
pub fn index(name: &str) -> Option<usize> {
    NAMES.iter().position(|n| n.eq_ignore_ascii_case(name))
}
pub fn apply(buffer: &mut Buffer, index: usize) {
    if index == 0 {
        return;
    }
    let palette = PALETTES[index % 5];
    let map = |color| {
        [BG, CREAM, AMBER, GREEN, Color::Rgb(107, 101, 86)]
            .iter()
            .position(|c| *c == color)
            .map(|i| palette[i])
            .unwrap_or(color)
    };
    for cell in &mut buffer.content {
        cell.fg = map(cell.fg);
        cell.bg = map(cell.bg);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_themes_are_selectable() {
        for (i, name) in NAMES.iter().enumerate() {
            assert_eq!(index(name), Some(i));
            let mut b = Buffer::empty(ratatui::layout::Rect::new(0, 0, 2, 1));
            b[(0, 0)].set_fg(AMBER).set_bg(BG);
            apply(&mut b, i);
            assert_eq!(b[(0, 0)].bg, PALETTES[i][0]);
        }
    }
}
