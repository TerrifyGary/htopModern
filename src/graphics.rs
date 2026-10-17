//! Background encoding for native terminal weather images.
//!
//! At most one encoding job is pending. Completed frames are reused between
//! updates, while resizing invalidates frames encoded for the previous area.

use image::{DynamicImage, RgbImage};
use ratatui::{layout::Rect, Frame};
use ratatui_image::{
    picker::{Picker, ProtocolType},
    protocol::Protocol,
    Image, Resize,
};
use std::{
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};

pub struct Graphics {
    picker: Option<Picker>,
    protocol: Option<Protocol>,
    pending: Option<Receiver<Result<Protocol, String>>>,
    area: Option<Rect>,
    last: Instant,
}
impl Graphics {
    pub fn new() -> Self {
        Self {
            picker: None,
            protocol: None,
            pending: None,
            area: None,
            last: Instant::now(),
        }
    }
    pub fn detect(&mut self) -> String {
        let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
        let name = format!("Weather graphics: {:?}", picker.protocol_type());
        if picker.protocol_type() != ProtocolType::Halfblocks {
            self.picker = Some(picker);
        }
        name
    }
    pub fn native(&self) -> bool {
        self.picker.is_some()
    }
    /// Poll completed encoding work and report whether another frame can be submitted.
    pub fn ready(&mut self, area: Rect) -> bool {
        if let Some(rx) = &self.pending {
            if let Ok(result) = rx.try_recv() {
                self.protocol = result.ok();
                self.pending = None;
                if self.protocol.is_none() {
                    self.picker = None;
                }
            }
        }
        if self.area != Some(area) {
            self.protocol = None;
        }
        self.pending.is_none()
            && (self.protocol.is_none() || self.last.elapsed() >= Duration::from_millis(250))
    }
    /// Encode a frame on a worker, keeping terminal input responsive.
    pub fn submit(&mut self, image: RgbImage, area: Rect) {
        let Some(picker) = self.picker.clone() else {
            return;
        };
        self.area = Some(area);
        self.last = Instant::now();
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        std::thread::spawn(move || {
            let result = picker
                .new_protocol(
                    DynamicImage::ImageRgb8(image),
                    area.into(),
                    Resize::Fit(None),
                )
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
    }
    pub fn render(&self, f: &mut Frame, area: Rect) {
        if let Some(protocol) = &self.protocol {
            f.render_widget(Image::new(protocol), area);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_encoding_and_resize_invalidation() {
        let mut g = Graphics::new();
        let mut picker = Picker::halfblocks();
        picker.set_protocol_type(ProtocolType::Iterm2);
        g.picker = Some(picker);
        let area = Rect::new(0, 0, 20, 10);
        g.submit(
            RgbImage::from_pixel(1024, 1024, image::Rgb([100, 140, 180])),
            area,
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while g.pending.is_some() && Instant::now() < deadline {
            g.ready(area);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(g.protocol.is_some());
        assert!(g.pending.is_none());
        assert!(g.ready(Rect::new(0, 0, 10, 5)));
        assert!(g.protocol.is_none());
    }
}
