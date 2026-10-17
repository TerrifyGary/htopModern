use crate::{block, App, AMBER, CREAM, GREEN};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};
use serde::Deserialize;
use std::{
    io::Read,
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};
const SCRIPT: &str = include_str!("apple_music.js");
#[derive(Debug, Clone, Deserialize)]
pub struct Track {
    title: String,
    artist: String,
    playing: bool,
    position: f64,
    duration: f64,
    #[serde(default)]
    lyrics: Option<String>,
}
#[derive(Deserialize)]
pub struct Reply {
    track: Option<Track>,
    pub status: String,
}
#[derive(Clone, Copy)]
pub enum Action {
    Read,
    Previous,
    Toggle,
    Next,
}
impl Action {
    fn name(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Previous => "previous",
            Self::Toggle => "toggle",
            Self::Next => "next",
        }
    }
}
pub fn query(action: Action) -> Result<Reply, String> {
    let mut child = Command::new("/usr/bin/osascript")
        .args(["-l", "JavaScript", "-e", SCRIPT, action.name()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Apple Music: {e}"))?;
    let stdout = child.stdout.take().ok_or("Could not read Music output")?;
    let reader = std::thread::spawn(move || {
        let mut output = String::new();
        stdout
            .take(262144)
            .read_to_string(&mut output)
            .map(|_| output)
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {}
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.to_string());
            }
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Music timed out · check Automation permission; R retries".into());
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let output = reader
        .join()
        .map_err(|_| "Music output reader failed")?
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(
            "Music unavailable · allow terminal → Music in macOS Automation; R retries".into(),
        );
    }
    let reply: Reply = serde_json::from_str(output.trim())
        .map_err(|_| "Could not read Apple Music playback".to_string())?;
    if let Some(track) = &reply.track {
        if !track.position.is_finite() || !track.duration.is_finite() {
            return Err("Invalid playback timing".into());
        }
    }
    Ok(reply)
}
pub struct Music {
    pub selected: usize,
    track: Option<Track>,
    pub status: String,
    pending: Option<Receiver<Result<Reply, String>>>,
    last: Instant,
    observed: Instant,
    failed: bool,
    spin_phase: f64,
    spin_updated: Instant,
    pub lyrics_scroll: usize,
    pub show_lyrics: bool,
}
impl Music {
    pub fn new() -> Self {
        Self {
            selected: 1,
            track: None,
            status: "Apple Music · waiting for connection".into(),
            pending: None,
            last: Instant::now(),
            observed: Instant::now(),
            failed: false,
            spin_phase: 0.,
            spin_updated: Instant::now(),
            lyrics_scroll: 0,
            show_lyrics: true,
        }
    }
    pub fn request(&mut self, action: Action) {
        if self.pending.is_some() {
            return;
        }
        self.last = Instant::now();
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(query(action));
        });
    }
    fn advance_spin(&mut self, seconds: f64) {
        if self.track.as_ref().is_some_and(|t| t.playing) {
            self.spin_phase = (self.spin_phase + seconds * 1.4) % std::f64::consts::TAU;
        }
    }
    fn apply(&mut self, result: Result<Reply, String>) {
        self.advance_spin(self.spin_updated.elapsed().as_secs_f64());
        self.spin_updated = Instant::now();
        match result {
            Ok(reply) => {
                if self.track.as_ref().map(|t| (&t.title, &t.artist))
                    != reply.track.as_ref().map(|t| (&t.title, &t.artist))
                {
                    self.lyrics_scroll = 0;
                }
                self.track = reply.track;
                self.status = reply.status;
                self.observed = Instant::now();
                self.failed = false;
            }
            Err(error) => {
                self.track = None;
                self.status = error;
                self.failed = true;
            }
        }
    }
    pub fn tick(&mut self) {
        self.advance_spin(self.spin_updated.elapsed().as_secs_f64());
        self.spin_updated = Instant::now();
        let result = self.pending.as_ref().and_then(|rx| match rx.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(Err("Music worker disconnected · R retries".into()))
            }
            Err(_) => None,
        });
        if let Some(result) = result {
            self.pending = None;
            self.apply(result);
        }
        if self.pending.is_none() && !self.failed && self.last.elapsed() >= Duration::from_secs(2) {
            self.request(Action::Read);
        }
    }
    pub fn activate(&mut self) {
        self.request(match self.selected {
            0 => Action::Previous,
            1 => Action::Toggle,
            _ => Action::Next,
        });
    }
    pub fn scroll_lyrics(&mut self, down: bool) {
        let max = self
            .track
            .as_ref()
            .and_then(|t| t.lyrics.as_ref())
            .map(|s| s.lines().count().saturating_sub(1))
            .unwrap_or(0);
        self.lyrics_scroll = if down {
            (self.lyrics_scroll + 1).min(max)
        } else {
            self.lyrics_scroll.saturating_sub(1)
        };
    }
    fn position(&self) -> f64 {
        self.track
            .as_ref()
            .map(|t| {
                let p = t.position.max(0.)
                    + if t.playing {
                        self.observed.elapsed().as_secs_f64()
                    } else {
                        0.
                    };
                if t.duration > 0. {
                    p.min(t.duration)
                } else {
                    p
                }
            })
            .unwrap_or(0.)
    }
}
fn clock(seconds: f64) -> String {
    let s = seconds.max(0.) as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}
fn cd_color(x: f64, y: f64, angle: f64) -> Color {
    let radius = x.hypot(y);
    if !(0.12..=1.).contains(&radius) {
        return crate::BG;
    }
    if radius < 0.31 {
        return AMBER;
    }
    let phase = y.atan2(x) - angle;
    let highlight = (phase * 2.).cos().max(0.).powi(12);
    let ring = (radius * 110.).sin() * 5.;
    let base = 145. + highlight * 75. + ring;
    Color::Rgb(
        base as u8,
        (base + 7.).min(255.) as u8,
        (base + 13.).min(255.) as u8,
    )
}
fn disc(f: &mut Frame, area: Rect, angle: f64, aspect: f64) {
    let h = area.height.min((area.width as f64 / aspect) as u16);
    let w = (h as f64 * aspect).round().min(area.width as f64) as u16;
    if h == 0 || w == 0 {
        return;
    }
    let x0 = area.x + (area.width - w) / 2;
    for row in 0..h {
        for col in 0..w {
            let x = (col as f64 + 0.5) / w as f64 * 2. - 1.;
            let y = (row as f64 * 2. + 0.5) / (h as f64 * 2.) * 2. - 1.;
            let bottom = (row as f64 * 2. + 1.5) / (h as f64 * 2.) * 2. - 1.;
            f.buffer_mut()[(x0 + col, area.y + row)]
                .set_symbol("▀")
                .set_fg(cd_color(x, y, angle))
                .set_bg(cd_color(x, bottom, angle));
        }
    }
}
fn media_heights(inner: Rect, aspect: f64, show_lyrics: bool) -> (u16, u16) {
    // Give the disc the panel's full width, just as the LOFI scene does.
    // Reserve track details and controls before allocating artwork or lyrics.
    let minimum_lyrics = if show_lyrics && inner.height >= 23 {
        3
    } else {
        0
    };
    let available = inner.height.saturating_sub(13 + minimum_lyrics);
    let desired = (inner.width as f64 / aspect).floor() as u16;
    let cd = if available >= 4 {
        desired.min(available)
    } else {
        0
    };
    let lyrics = if minimum_lyrics > 0 {
        inner.height.saturating_sub(cd + 13).min(8)
    } else {
        0
    };
    (cd, lyrics)
}
pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let music = &app.music;
    let b = block(" MUSIC · M ", app.panel == 1);
    let inner = b.inner(area);
    f.render_widget(b, area);
    let (cd_height, lyrics_height) = media_heights(inner, app.cell_aspect, music.show_lyrics);
    let sections = Layout::vertical([
        Constraint::Length(cd_height),
        Constraint::Min(1),
        Constraint::Length(lyrics_height),
    ])
    .split(inner);
    if cd_height > 0 {
        disc(f, sections[0], music.spin_phase, app.cell_aspect);
    }
    let playing = music.track.as_ref().is_some_and(|t| t.playing);
    let symbols = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let time = app.started.elapsed().as_secs_f64();
    let bars = (0..usize::from(inner.width).min(16))
        .map(|i| {
            let n = if playing {
                ((time * 3. + i as f64 * 1.9).sin().abs() * 7.) as usize
            } else {
                0
            };
            symbols[n].to_string()
        })
        .collect::<Vec<_>>()
        .join(" ");
    let mut lines = vec![
        Line::styled(bars, Style::default().fg(GREEN)),
        Line::from(""),
    ];
    if let Some(track) = &music.track {
        lines.push(Line::from(track.title.clone()));
        lines.push(Line::from(track.artist.clone()));
        lines.push(Line::from(format!(
            "{} / {}",
            clock(music.position()),
            if track.duration > 0. {
                clock(track.duration)
            } else {
                "LIVE".into()
            }
        )));
    } else {
        lines.push(Line::from("No active track"));
    }
    lines.push(Line::from(""));
    let labels = ["[⏮]", if playing { "[⏸]" } else { "[▶]" }, "[⏭]"];
    lines.push(Line::from(
        labels
            .iter()
            .enumerate()
            .map(|(i, label)| {
                Span::styled(
                    format!("{label} "),
                    Style::default().fg(if app.panel == 1 && music.selected == i {
                        AMBER
                    } else {
                        CREAM
                    }),
                )
            })
            .collect::<Vec<_>>(),
    ));
    lines.push(Line::from("←→ select · Enter activate"));
    lines.push(Line::from("Apple Music · R refresh"));
    lines.push(Line::from(music.status.clone()));
    lines.push(Line::from(""));
    lines.push(Line::from("L lyrics · ↑↓ scroll"));
    f.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }),
        sections[1],
    );
    if lyrics_height > 0 {
        let lyrics = music
            .track
            .as_ref()
            .and_then(|t| t.lyrics.as_deref())
            .unwrap_or("Lyrics unavailable for this track");
        let content = lyrics
            .lines()
            .skip(music.lyrics_scroll)
            .collect::<Vec<_>>()
            .join("\n");
        f.render_widget(
            Paragraph::new(content)
                .block(block(" LYRICS · ↑↓ ", false))
                .wrap(Wrap { trim: false }),
            sections[2],
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cd_uses_full_panel_width_when_space_allows() {
        let (cd, lyrics) = media_heights(Rect::new(0, 0, 46, 44), 2.3, true);
        assert_eq!(cd, 20);
        assert!(lyrics >= 3);
        assert!(cd + lyrics + 13 <= 44);
        let (small, _) = media_heights(Rect::new(0, 0, 25, 8), 2.3, true);
        assert_eq!(small, 0);
    }
    #[test]
    fn disc_freezes_on_pause_and_stop() {
        let mut m = Music::new();
        m.track = Some(Track {
            title: "Track".into(),
            artist: "Artist".into(),
            playing: true,
            position: 0.,
            duration: 100.,
            lyrics: Some("one\ntwo\nthree".into()),
        });
        m.advance_spin(1.);
        let spinning = m.spin_phase;
        assert!(spinning > 0.);
        m.track.as_mut().unwrap().playing = false;
        m.advance_spin(5.);
        assert_eq!(m.spin_phase, spinning);
        m.scroll_lyrics(true);
        assert_eq!(m.lyrics_scroll, 1);
        m.track = None;
        m.advance_spin(5.);
        assert_eq!(m.spin_phase, spinning);
    }
    #[test]
    fn timing_and_error_state() {
        assert_eq!(clock(102.), "1:42");
        assert_eq!(clock(3661.), "1:01:01");
        let mut m = Music::new();
        m.apply(Ok(Reply {
            track: Some(Track {
                title: "日本語".into(),
                artist: "Artist".into(),
                playing: false,
                position: 42.,
                duration: 100.,
                lyrics: None,
            }),
            status: "Paused".into(),
        }));
        assert_eq!(m.position(), 42.);
        m.apply(Err("Denied".into()));
        assert!(m.track.is_none());
        assert!(m.failed);
    }
    #[test]
    fn panel_fits_small_and_large_sizes() {
        let mut app =
            App::new(std::env::temp_dir().join("htopmodern-music-render/todos.json")).unwrap();
        app.panel = 1;
        for (w, h) in [(1, 1), (25, 8), (40, 25)] {
            let mut term =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, h)).unwrap();
            term.draw(|f| draw(f, f.area(), &app)).unwrap();
        }
    }
}
