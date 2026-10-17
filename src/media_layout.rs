//! Shared animation and information regions for LOFI, Music, and Weather.
//!
//! Centralizing these rectangles keeps dashed dividers and text baselines aligned.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::Paragraph,
    Frame,
};

/// Return the animation region and the information region below it.
pub fn regions(inner: Rect) -> (Rect, Rect) {
    let visual_height = if inner.height >= 20 {
        inner.height - 12
    } else {
        inner.height / 2
    };
    let content = Rect::new(
        inner.x,
        inner.y + visual_height.min(2),
        inner.width,
        visual_height.saturating_sub(3),
    );
    let info = Rect::new(
        inner.x,
        inner.y + visual_height,
        inner.width,
        inner.height - visual_height,
    );
    (content, info)
}
/// Build a dashed separator clipped to the available terminal columns.
pub fn divider(width: u16) -> Line<'static> {
    Line::styled(
        "- ".repeat(usize::from(width).div_ceil(2))
            .chars()
            .take(usize::from(width))
            .collect::<String>(),
        Style::default().fg(Color::Rgb(107, 101, 86)),
    )
}
/// Draw shared separators and return the regions available to panel content.
pub fn frame(f: &mut Frame, inner: Rect) -> (Rect, Rect) {
    let (content, info) = regions(inner);
    if inner.height > 0 {
        f.render_widget(
            Paragraph::new(divider(inner.width)),
            Rect::new(inner.x, inner.y, inner.width, 1),
        );
    }
    if info.y > inner.y {
        f.render_widget(
            Paragraph::new(divider(inner.width)),
            Rect::new(inner.x, info.y - 1, inner.width, 1),
        );
    }
    (content, info)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_baselines_fit_every_panel() {
        for h in 0..60 {
            let inner = Rect::new(2, 3, 40, h);
            let (art, info) = regions(inner);
            assert!(art.bottom() <= inner.bottom());
            assert!(info.bottom() <= inner.bottom());
            if h >= 20 {
                assert_eq!(info.height, 12);
            }
        }
    }
}
