use crate::{block, App, AMBER, CREAM, GREEN};
use ratatui::{
    layout::Rect,
    style::Style,
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
    let mut output = String::new();
    if let Some(stdout) = child.stdout.take() {
        stdout
            .take(65536)
            .read_to_string(&mut output)
            .map_err(|e| e.to_string())?;
    }
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
    fn apply(&mut self, result: Result<Reply, String>) {
        match result {
            Ok(reply) => {
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
pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let music = &app.music;
    let b = block(" MUSIC · M ", app.panel == 1);
    let inner = b.inner(area);
    f.render_widget(b, area);
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
    lines.push(Line::from("YouTube / Firefox: pending"));
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}
#[cfg(test)]
mod tests {
    use super::*;
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
