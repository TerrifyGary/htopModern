use crate::{block, App, AMBER, CREAM, GREEN};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};
use serde::Deserialize;
use std::{
    collections::VecDeque,
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
fn script_output(script: &str, language: &str, argument: &str) -> Result<String, String> {
    let mut child = Command::new("/usr/bin/osascript")
        .args(["-l", language, "-e", script, argument])
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
        std::thread::sleep(Duration::from_millis(5));
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
    Ok(output)
}
pub fn query(action: Action) -> Result<Reply, String> {
    let output = script_output(SCRIPT, "JavaScript", action.name())?;
    let reply: Reply = serde_json::from_str(output.trim())
        .map_err(|_| "Could not read Apple Music playback".to_string())?;
    if let Some(track) = &reply.track {
        if !track.position.is_finite() || !track.duration.is_finite() {
            return Err("Invalid playback timing".into());
        }
    }
    Ok(reply)
}
fn control(action: Action) -> Result<(), String> {
    let command = match action {
        Action::Previous => "previous track",
        Action::Toggle => "playpause",
        Action::Next => "next track",
        Action::Read => return Ok(()),
    };
    let script=format!("if application \"Music\" is running then\n tell application \"Music\" to {command}\nelse\n error \"Open Apple Music first\"\nend if");
    script_output(&script, "AppleScript", "").map(|_| ())
}
pub struct Music {
    pub selected: usize,
    track: Option<Track>,
    pub status: String,
    pending: Option<Receiver<Result<Reply, String>>>,
    last: Instant,
    commands: VecDeque<Action>,
    control_pending: Option<Receiver<Result<(), String>>>,
    epoch: u64,
    pending_epoch: u64,
    observed: Instant,
    failed: bool,
    spin_phase: f64,
    equalizer_color: usize,
    spin_updated: Instant,
}
impl Music {
    pub fn new() -> Self {
        Self {
            selected: 1,
            track: None,
            status: "Apple Music · waiting for connection".into(),
            pending: None,
            last: Instant::now(),
            commands: VecDeque::new(),
            control_pending: None,
            epoch: 0,
            pending_epoch: 0,
            observed: Instant::now(),
            failed: false,
            spin_phase: 0.,
            equalizer_color: 0,
            spin_updated: Instant::now(),
        }
    }
    pub fn request(&mut self, action: Action) {
        if !matches!(action, Action::Read) {
            self.commands.push_back(action);
            self.epoch = self.epoch.wrapping_add(1);
            self.start_control();
            return;
        }
        if self.pending.is_some() {
            return;
        }
        self.last = Instant::now();
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        self.pending_epoch = self.epoch;
        std::thread::spawn(move || {
            let _ = tx.send(query(action));
        });
    }
    fn start_control(&mut self) {
        if self.control_pending.is_some() {
            return;
        }
        if let Some(action) = self.commands.pop_front() {
            let (tx, rx) = mpsc::channel();
            self.control_pending = Some(rx);
            std::thread::spawn(move || {
                let _ = tx.send(control(action));
            });
        }
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
                if let (Some(old), Some(new)) = (&self.track, &reply.track) {
                    if old.title != new.title || old.artist != new.artist {
                        self.equalizer_color = (self.equalizer_color + 1) % 6;
                    }
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
        let command_result = self
            .control_pending
            .as_ref()
            .and_then(|rx| match rx.try_recv() {
                Ok(r) => Some(r),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("Music control disconnected · R retries".into()))
                }
                Err(_) => None,
            });
        if let Some(result) = command_result {
            self.control_pending = None;
            match result {
                Ok(()) => {
                    self.failed = false;
                    self.last = Instant::now() - Duration::from_secs(2);
                }
                Err(error) => self.apply(Err(error)),
            }
            self.start_control();
        }
        let result = self.pending.as_ref().and_then(|rx| match rx.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(Err("Music worker disconnected · R retries".into()))
            }
            Err(_) => None,
        });
        if let Some(result) = result {
            self.pending = None;
            if self.pending_epoch == self.epoch
                && self.control_pending.is_none()
                && self.commands.is_empty()
            {
                self.apply(result);
            }
        }
        if self.pending.is_none()
            && self.control_pending.is_none()
            && self.commands.is_empty()
            && !self.failed
            && self.last.elapsed() >= Duration::from_millis(750)
        {
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
fn media_heights(inner: Rect, aspect: f64) -> u16 {
    let available = inner.height.saturating_sub(22);
    let desired = (inner.width as f64 / aspect * 0.7).floor() as u16;
    if available >= 4 {
        desired.min(available)
    } else {
        0
    }
}
fn progress_line(width: usize, position: f64, duration: f64) -> String {
    if width == 0 {
        return String::new();
    }
    if duration <= 0. || !duration.is_finite() {
        return "─".repeat(width);
    }
    let progress = (position / duration).clamp(0., 1.);
    let index = (progress * (width - 1) as f64).round() as usize;
    (0..width)
        .map(|i| if i == index { '●' } else { '─' })
        .collect()
}
fn equalizer_lines(
    width: u16,
    height: usize,
    playing: bool,
    time: f64,
    color: Color,
) -> Vec<Line<'static>> {
    let count = usize::from(width).div_ceil(2);
    let levels: Vec<_> = (0..count)
        .map(|i| {
            if playing {
                1 + ((time * 3. + i as f64 * 1.9).sin().abs() * (height - 1) as f64).round()
                    as usize
            } else {
                1
            }
        })
        .collect();
    (0..height)
        .map(|row| {
            let text = levels
                .iter()
                .map(|&level| if level >= height - row { "▏" } else { " " })
                .collect::<Vec<_>>()
                .join(" ");
            Line::styled(text, Style::default().fg(color))
        })
        .collect()
}
pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let music = &app.music;
    let b = block(" MUSIC · M ", app.panel == 1);
    let inner = b.inner(area);
    f.render_widget(b, area);
    let (visual, info) = crate::media_layout::frame(f, inner);
    let bar_rows = if visual.height >= 9 { 4 } else { 1 };
    let equalizer_height = (bar_rows + 2).min(usize::from(visual.height)) as u16;
    let cd_space = visual.height.saturating_sub(equalizer_height + 1);
    let cd_height = media_heights(inner, app.cell_aspect).min(cd_space);
    if cd_height > 0 {
        disc(
            f,
            Rect::new(
                visual.x,
                visual.y + (cd_space - cd_height) / 2,
                visual.width,
                cd_height,
            ),
            music.spin_phase,
            app.cell_aspect,
        );
    }
    let bars_y = visual.y + visual.height.saturating_sub(equalizer_height);
    if cd_space > 0 {
        f.render_widget(
            Paragraph::new(crate::media_layout::divider(inner.width)),
            Rect::new(visual.x, bars_y.saturating_sub(1), visual.width, 1),
        );
    }
    let playing = music.track.as_ref().is_some_and(|t| t.playing);

    let mut lines = equalizer_lines(
        inner.width,
        bar_rows,
        playing,
        app.started.elapsed().as_secs_f64(),
        [
            GREEN,
            Color::Rgb(129, 182, 176),
            Color::Rgb(205, 167, 109),
            Color::Rgb(184, 155, 200),
            Color::Rgb(208, 148, 153),
            Color::Rgb(139, 171, 207),
        ][music.equalizer_color],
    );
    lines.insert(0, Line::from(""));
    lines.push(Line::from(""));
    f.render_widget(
        Paragraph::new(lines),
        Rect::new(visual.x, bars_y, visual.width, equalizer_height),
    );
    let mut lines = vec![Line::from("")];
    if let Some(track) = &music.track {
        lines.push(
            Line::styled(
                track.title.clone(),
                Style::default()
                    .fg(CREAM)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            )
            .centered(),
        );
        lines.push(Line::from(track.artist.clone()).centered());
        lines.push(Line::from(""));
        let width = usize::from(inner.width.saturating_sub(4)).min(48);
        lines.push(
            Line::styled(
                progress_line(width, music.position(), track.duration),
                Style::default().fg(AMBER),
            )
            .centered(),
        );
        lines.push(
            Line::from(format!(
                "{} / {}",
                clock(music.position()),
                if track.duration > 0. {
                    clock(track.duration)
                } else {
                    "LIVE".into()
                }
            ))
            .centered(),
        );
    } else {
        lines.push(Line::from("No active track").centered());
    }
    lines.push(Line::from(""));
    let labels = ["[⏮]", if playing { "[⏸]" } else { "[▶]" }, "[⏭]"];
    lines.push(
        Line::from(
            labels
                .iter()
                .enumerate()
                .map(|(i, label)| {
                    Span::styled(
                        format!("{}{label}", if i > 0 { "   " } else { "" }),
                        Style::default().fg(if app.panel == 1 && music.selected == i {
                            AMBER
                        } else {
                            CREAM
                        }),
                    )
                })
                .collect::<Vec<_>>(),
        )
        .centered(),
    );
    if music.failed || music.track.is_none() {
        lines.push(Line::from(""));
        lines.push(Line::from(music.status.clone()).centered());
    }
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), info);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn progress_dot_moves_and_clamps() {
        assert_eq!(progress_line(5, 0., 100.), "●────");
        assert_eq!(progress_line(5, 50., 100.), "──●──");
        assert_eq!(progress_line(5, 200., 100.), "────●");
        assert_eq!(progress_line(0, 0., 100.), "");
        assert_eq!(progress_line(5, 0., 0.), "─────");
    }
    #[test]
    fn controls_are_retained_and_stale_reads_cannot_undo_them() {
        let mut music = Music::new();
        music.track = Some(Track {
            title: "Song".into(),
            artist: "Artist".into(),
            playing: true,
            position: 0.,
            duration: 100.,
        });
        let (_control_tx, control_rx) = mpsc::channel();
        music.control_pending = Some(control_rx);
        let (read_tx, read_rx) = mpsc::channel();
        music.pending = Some(read_rx);
        music.pending_epoch = 0;
        music.request(Action::Toggle);
        music.request(Action::Next);
        assert_eq!(music.commands.len(), 2);
        assert_eq!(music.epoch, 2);
        read_tx
            .send(Ok(Reply {
                track: None,
                status: "old".into(),
            }))
            .unwrap();
        music.tick();
        assert!(music.track.as_ref().unwrap().playing);
        assert_eq!(music.commands.len(), 2);
    }
    #[test]
    fn equalizer_is_tall_thin_and_fills_width() {
        let lines = equalizer_lines(47, 4, true, 1., GREEN);
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[3].spans[0].content.matches('▏').count(), 24);
        assert!(lines
            .iter()
            .all(|line| line.spans[0].content.chars().count() <= 47));
    }
    #[test]
    fn cd_uses_seventy_percent_of_full_width() {
        let cd = media_heights(Rect::new(0, 0, 46, 44), 2.3);
        assert_eq!(cd, 14);
        assert!(cd + 18 <= 44);
        let small = media_heights(Rect::new(0, 0, 25, 8), 2.3);
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
        });
        m.advance_spin(1.);
        let spinning = m.spin_phase;
        assert!(spinning > 0.);
        m.track.as_mut().unwrap().playing = false;
        m.advance_spin(5.);
        assert_eq!(m.spin_phase, spinning);
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
