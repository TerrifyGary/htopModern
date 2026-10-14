use crate::{block, App, AMBER};
use chrono::{Local, Timelike, Utc};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    symbols::Marker,
    widgets::{
        canvas::{Canvas, Circle, Context, Line as Stroke},
        Paragraph, Wrap,
    },
    Frame,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub query: String,
    pub location: String,
    pub temperature: f64,
    pub code: u32,
    pub wind: f64,
    pub day: bool,
    pub fetched: i64,
}
impl Snapshot {
    pub fn condition(&self) -> &'static str {
        condition(self.code, self.wind)
    }
}
fn condition(code: u32, wind: f64) -> &'static str {
    match code {
        71 | 73 | 75 | 77 | 85 | 86 => "Snowy",
        51 | 53 | 55 | 56 | 57 | 61 | 63 | 65 | 66 | 67 | 80 | 81 | 82 | 95 | 96 | 99 => "Rainy",
        _ if wind >= 25. => "Windy",
        0 => "Sunny",
        1..=3 | 45 | 48 => "Cloudy",
        _ => "Unknown",
    }
}
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
fn json(url: &str) -> Result<serde_json::Value, String> {
    let out = Command::new("/usr/bin/curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--max-time",
            "12",
            "--connect-timeout",
            "5",
            url,
        ])
        .output()
        .map_err(|e| format!("Weather request: {e}"))?;
    if !out.status.success() {
        return Err("Weather unavailable; check connection (R retries)".into());
    }
    serde_json::from_slice(&out.stdout).map_err(|_| "Invalid weather response".into())
}
pub fn fetch(query: &str) -> Result<Snapshot, String> {
    let geo = json(&format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=en&format=json",
        encode(query)
    ))?;
    let place = geo["results"]
        .as_array()
        .and_then(|v| v.first())
        .ok_or("Location not found; try a city name")?;
    let lat = place["latitude"].as_f64().ok_or("Invalid latitude")?;
    let lon = place["longitude"].as_f64().ok_or("Invalid longitude")?;
    let data=json(&format!("https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}&current=temperature_2m,weather_code,wind_speed_10m,is_day&temperature_unit=celsius&wind_speed_unit=kmh&timezone=auto"))?;
    let c = &data["current"];
    let snapshot = Snapshot {
        query: query.into(),
        location: format!(
            "{}, {}",
            place["name"].as_str().unwrap_or(query),
            place["country"].as_str().unwrap_or("")
        ),
        temperature: c["temperature_2m"].as_f64().ok_or("Missing temperature")?,
        code: c["weather_code"]
            .as_u64()
            .ok_or("Missing weather condition")? as u32,
        wind: c["wind_speed_10m"].as_f64().ok_or("Missing wind speed")?,
        day: c["is_day"].as_u64().ok_or("Missing daylight status")? == 1,
        fetched: Utc::now().timestamp(),
    };
    Ok(snapshot)
}
type Reply = (u64, Result<Snapshot, String>);
pub struct Weather {
    pub query: String,
    pub snapshot: Option<Snapshot>,
    pub input: Option<String>,
    pub status: String,
    path: PathBuf,
    tx: Sender<Reply>,
    rx: Receiver<Reply>,
    generation: u64,
    loading: bool,
    last: Instant,
}
impl Weather {
    pub fn new(path: PathBuf) -> Self {
        let (tx, rx) = mpsc::channel();
        let snapshot = fs::read(&path)
            .ok()
            .and_then(|v| serde_json::from_slice::<Snapshot>(&v).ok());
        let query = snapshot
            .as_ref()
            .map(|s| s.query.clone())
            .unwrap_or_else(|| "Taipei".into());
        Self {
            query,
            snapshot,
            input: None,
            status: "Cached weather · awaiting refresh".into(),
            path,
            tx,
            rx,
            generation: 0,
            loading: false,
            last: Instant::now(),
        }
    }
    pub fn request(&mut self) {
        if self.loading {
            return;
        }
        self.generation += 1;
        let id = self.generation;
        let tx = self.tx.clone();
        let query = self.query.clone();
        self.loading = true;
        self.last = Instant::now();
        self.status = "Loading weather…".into();
        std::thread::spawn(move || {
            let _ = tx.send((id, fetch(&query)));
        });
    }
    pub fn set_query(&mut self, query: String) {
        if query != self.query {
            self.snapshot = None;
            self.query = query;
            self.generation += 1;
            self.loading = false;
        }
        self.request();
    }
    fn apply(&mut self, id: u64, result: Result<Snapshot, String>) {
        if id != self.generation {
            return;
        }
        self.loading = false;
        match result {
            Ok(snapshot) => {
                self.status = "Open-Meteo · updated just now".into();
                let save = (|| -> std::io::Result<()> {
                    if let Some(p) = self.path.parent() {
                        fs::create_dir_all(p)?;
                    }
                    let temp = self.path.with_extension("json.tmp");
                    fs::write(&temp, serde_json::to_vec(&snapshot)?)?;
                    fs::rename(temp, &self.path)
                })();
                if save.is_err() {
                    self.status = "Live weather · cache could not be saved".into();
                }
                self.snapshot = Some(snapshot);
            }
            Err(error) => {
                self.status = format!(
                    "{error}{}",
                    if self.snapshot.is_some() {
                        " · showing cached data"
                    } else {
                        ""
                    }
                )
            }
        }
    }
    pub fn tick(&mut self) {
        while let Ok((id, result)) = self.rx.try_recv() {
            self.apply(id, result);
        }
        if !self.loading && self.last.elapsed() >= Duration::from_secs(600) {
            self.request();
        }
    }
}
fn fill(ctx: &mut Context<'_>, x: f64, y: f64, w: f64, h: f64, color: Color) {
    let mut n = y;
    while n <= y + h {
        ctx.draw(&Stroke {
            x1: x,
            y1: n,
            x2: x + w,
            y2: n,
            color,
        });
        n += 0.5;
    }
}
pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let weather = &app.weather;
    let b = block(" WEATHER · W ", app.panel == 3);
    let inner = b.inner(area);
    f.render_widget(b, area);
    if inner.height < 2 {
        return;
    }
    let title = weather
        .input
        .as_ref()
        .map(|s| format!("Location: {s}▏"))
        .unwrap_or_else(|| format!("[Location: {}]", weather.query));
    let snapshot = weather.snapshot.as_ref();
    let mode = snapshot.map(|s| s.condition()).unwrap_or("Unknown");
    let day = snapshot
        .map(|s| s.day)
        .unwrap_or_else(|| (6..18).contains(&Local::now().hour()));
    let info = snapshot
        .map(|s| {
            format!(
                "{} · {:.1}°C · {}",
                s.condition(),
                s.temperature,
                if s.day { "Day" } else { "Night" }
            )
        })
        .unwrap_or_else(|| "Temperature: unavailable".into());
    let details = if let Some(snapshot) = snapshot {
        format!("{}\n{info}", snapshot.location)
    } else {
        info
    };
    f.render_widget(
        Paragraph::new(title).style(Style::default().fg(AMBER)),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );
    f.render_widget(
        Paragraph::new(details).wrap(Wrap { trim: false }),
        Rect::new(inner.x, inner.y + 1, inner.width, 2.min(inner.height - 1)),
    );
    let available = inner.height.saturating_sub(6);
    let h = available.min(inner.width / 2);
    let art = Rect::new(inner.x + (inner.width - h * 2) / 2, inner.y + 3, h * 2, h);
    let t = app.started.elapsed().as_secs_f64();
    let canvas = Canvas::default()
        .marker(Marker::HalfBlock)
        .x_bounds([0., 100.])
        .y_bounds([0., 100.])
        .paint(|ctx| {
            fill(
                ctx,
                0.,
                0.,
                100.,
                100.,
                if day {
                    Color::Rgb(181, 200, 189)
                } else {
                    Color::Rgb(40, 55, 76)
                },
            );
            fill(ctx, 0., 0., 100., 12., Color::Rgb(112, 138, 105));
            ctx.draw(&Circle {
                x: 20.,
                y: 82.,
                radius: 7.,
                color: Color::Rgb(243, 209, 130),
            });
            if !day {
                fill(ctx, 21., 79., 7., 10., Color::Rgb(40, 55, 76));
                for (x, y) in [(45., 90.), (80., 75.), (65., 85.)] {
                    fill(ctx, x, y, 1., 1., Color::Rgb(238, 240, 225));
                }
            }
            if mode != "Sunny" {
                let x = if mode == "Windy" {
                    65. + (t * 0.6).sin() * 16.
                } else {
                    65. + (t * 0.2).sin() * 8.
                };
                for dx in [-7., 0., 7.] {
                    ctx.draw(&Circle {
                        x: x + dx,
                        y: 77.,
                        radius: 6.,
                        color: Color::Rgb(225, 231, 221),
                    });
                }
            }
            if mode == "Rainy" || mode == "Snowy" {
                for i in 0..14 {
                    let x = (i * 23 % 100) as f64;
                    let y =
                        95. - ((t * if mode == "Rainy" { 35. } else { 10. } + i as f64 * 9.) % 80.);
                    if mode == "Rainy" {
                        ctx.draw(&Stroke {
                            x1: x,
                            y1: y,
                            x2: x - 2.,
                            y2: y - 5.,
                            color: Color::Rgb(200, 216, 225),
                        });
                    } else {
                        fill(ctx, x, y, 1.5, 1.5, Color::Rgb(235, 238, 235));
                    }
                }
            }
            if mode == "Windy" {
                for i in 0..7 {
                    let x = (t * 22. + i as f64 * 19.) % 110. - 10.;
                    let y = 25. + (i % 4) as f64 * 12.;
                    ctx.draw(&Stroke {
                        x1: x.max(0.),
                        y1: y,
                        x2: (x + 14.).min(100.),
                        y2: y,
                        color: Color::Rgb(211, 225, 216),
                    });
                    fill(
                        ctx,
                        (x + 8.).clamp(0., 98.),
                        y + (t + i as f64).sin() * 2.,
                        2.,
                        1.,
                        Color::Rgb(183, 163, 105),
                    );
                }
            }
            if mode == "Sunny" && day {
                let pulse = (t * 1.5).sin() * 1.5;
                for n in 0..8 {
                    let angle = n as f64 * std::f64::consts::TAU / 8.;
                    ctx.draw(&Stroke {
                        x1: 20. + angle.sin() * 10.,
                        y1: 82. + angle.cos() * 10.,
                        x2: 20. + angle.sin() * (14. + pulse),
                        y2: 82. + angle.cos() * (14. + pulse),
                        color: Color::Rgb(243, 209, 130),
                    });
                }
            }
            if mode == "Rainy" {
                for i in 0..4 {
                    let radius = 2. + ((t * 5. + i as f64) % 5.);
                    ctx.draw(&Stroke {
                        x1: 15. + i as f64 * 22. - radius,
                        y1: 10.,
                        x2: 15. + i as f64 * 22. + radius,
                        y2: 10.,
                        color: Color::Rgb(148, 181, 195),
                    });
                }
            }
            if mode == "Snowy" {
                fill(ctx, 0., 0., 100., 10., Color::Rgb(219, 228, 223));
            }
        });
    if h > 0 {
        f.render_widget(canvas, art);
    }
    let bottom = inner.y + inner.height.saturating_sub(3);
    let age = snapshot
        .map(|s| {
            format!(
                " · {}m old",
                (Utc::now().timestamp() - s.fetched).max(0) / 60
            )
        })
        .unwrap_or_default();
    let hint = if weather.input.is_some() {
        "Enter saves · Esc cancels"
    } else {
        "Enter/E location · R refresh"
    };
    f.render_widget(
        Paragraph::new(format!("{}{age}\n{hint}", weather.status)).wrap(Wrap { trim: false }),
        Rect::new(inner.x, bottom, inner.width, 3.min(inner.height)),
    );
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn weather_scenes_render_in_each_condition_and_small_panels() {
        let mut app =
            App::new(std::env::temp_dir().join("htopmodern-weather-render/todos.json")).unwrap();
        app.panel = 3;
        for (code, wind) in [(0, 0.), (0, 30.), (3, 0.), (61, 0.), (75, 0.)] {
            for day in [true, false] {
                app.weather.snapshot = Some(Snapshot {
                    query: "Taipei".into(),
                    location: "Taipei".into(),
                    temperature: 25.9,
                    code,
                    wind,
                    day,
                    fetched: Utc::now().timestamp(),
                });
                for (w, h) in [(1, 1), (25, 8), (40, 25)] {
                    let mut term =
                        ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, h)).unwrap();
                    term.draw(|f| draw(f, f.area(), &app)).unwrap();
                }
            }
        }
    }
    #[test]
    fn conditions_and_encoding() {
        assert_eq!(condition(75, 40.), "Snowy");
        assert_eq!(condition(95, 40.), "Rainy");
        assert_eq!(condition(0, 30.), "Windy");
        assert_eq!(condition(0, 0.), "Sunny");
        assert_eq!(condition(3, 0.), "Cloudy");
        assert_eq!(
            encode("New York & 東京"),
            "New%20York%20%26%20%E6%9D%B1%E4%BA%AC"
        );
    }
    #[test]
    fn stale_reply_is_ignored_and_cache_survives_failure() {
        let path = std::env::temp_dir().join("htopmodern-weather-test.json");
        let mut w = Weather::new(path);
        w.generation = 2;
        w.apply(1, Err("old".into()));
        assert_ne!(w.status, "old");
        w.snapshot = Some(Snapshot {
            query: "Taipei".into(),
            location: "Taipei".into(),
            temperature: 25.,
            code: 0,
            wind: 0.,
            day: true,
            fetched: 0,
        });
        w.apply(2, Err("Offline".into()));
        assert!(w.snapshot.is_some());
        assert!(w.status.contains("cached"));
    }
}
