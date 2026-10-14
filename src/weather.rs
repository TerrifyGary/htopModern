use crate::{block, App, AMBER};
use chrono::{Local, Timelike, Utc};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{canvas::Line as Stroke, Paragraph, Wrap},
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
const SAMPLES: usize = 4;
struct Raster {
    width: usize,
    height: usize,
    pixels: Vec<Color>,
}
impl Raster {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![Color::Reset; width * height],
        }
    }
    fn draw(&mut self, line: &Stroke) {
        let steps = (((line.x2 - line.x1).abs() * self.width as f64 / 100.)
            .max((line.y2 - line.y1).abs() * self.height as f64 / 100.)
            * 2.)
            .ceil() as usize;
        for n in 0..=steps.max(1) {
            let t = n as f64 / steps.max(1) as f64;
            self.dot(
                line.x1 + (line.x2 - line.x1) * t,
                line.y1 + (line.y2 - line.y1) * t,
                line.color,
            );
        }
    }
    fn dot(&mut self, x: f64, y: f64, color: Color) {
        let col = (x / 100. * self.width.saturating_sub(1) as f64).round() as usize;
        let row = ((100. - y) / 100. * self.height.saturating_sub(1) as f64).round() as usize;
        if col < self.width && row < self.height {
            self.pixels[row * self.width + col] = color;
        }
    }
    fn average(&self, x: usize, y: usize) -> Color {
        let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
        for dy in 0..SAMPLES {
            for dx in 0..SAMPLES {
                if let Color::Rgb(cr, cg, cb) = self.pixels[(y + dy) * self.width + x + dx] {
                    r += (cr as u32).pow(2);
                    g += (cg as u32).pow(2);
                    b += (cb as u32).pow(2);
                }
            }
        }
        Color::Rgb(
            (r as f64 / (SAMPLES * SAMPLES) as f64).sqrt() as u8,
            (g as f64 / (SAMPLES * SAMPLES) as f64).sqrt() as u8,
            (b as f64 / (SAMPLES * SAMPLES) as f64).sqrt() as u8,
        )
    }
}
fn fill(ctx: &mut Raster, x: f64, y: f64, w: f64, h: f64, color: Color) {
    let x0 = (x / 100. * ctx.width as f64).floor().max(0.) as usize;
    let x1 = ((x + w) / 100. * ctx.width as f64).ceil().max(0.) as usize;
    let y0 = ((100. - y - h) / 100. * ctx.height as f64).floor().max(0.) as usize;
    let y1 = ((100. - y) / 100. * ctx.height as f64).ceil().max(0.) as usize;
    for row in y0..y1.max(y0 + 1).min(ctx.height) {
        for col in x0..x1.max(x0 + 1).min(ctx.width) {
            ctx.pixels[row * ctx.width + col] = color;
        }
    }
}
fn disk(ctx: &mut Raster, x: f64, y: f64, r: f64, color: Color) {
    for row in 0..ctx.height {
        let py = 100. - (row as f64 + 0.5) / ctx.height as f64 * 100.;
        if (py - y).abs() <= r {
            let half = (r * r - (py - y).powi(2)).sqrt();
            let x0 = ((x - half) / 100. * ctx.width as f64).floor().max(0.) as usize;
            let x1 = ((x + half) / 100. * ctx.width as f64).ceil().max(0.) as usize;
            for col in x0..x1.min(ctx.width) {
                ctx.pixels[row * ctx.width + col] = color;
            }
        }
    }
}
fn cloud(ctx: &mut Raster, x: f64, y: f64, scale: f64, color: Color) {
    fill(
        ctx,
        x - 15. * scale,
        y - 4. * scale,
        30. * scale,
        7. * scale,
        color,
    );
    for (dx, dy, r) in [(-11., 0., 6.), (-3., 4., 9.), (7., 3., 7.), (13., 0., 5.)] {
        disk(ctx, x + dx * scale, y + dy * scale, r * scale, color);
    }
}
fn square_viewport(area: Rect, aspect: f64) -> Rect {
    let height = area.height.min((area.width as f64 / aspect).floor() as u16);
    let width = (height as f64 * aspect).round().min(area.width as f64) as u16;
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
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
    let art = square_viewport(
        Rect::new(inner.x, inner.y + 3, inner.width, available),
        app.cell_aspect,
    );
    let h = art.height;
    let t = app.started.elapsed().as_secs_f64();
    if h > 0 {
        let mut raster = Raster::new(
            usize::from(art.width) * SAMPLES,
            usize::from(art.height) * SAMPLES * 2,
        );
        let ctx = &mut raster;
        let storm = mode == "Rainy";
        let sky = if !day {
            Color::Rgb(36, 45, 66)
        } else if storm {
            Color::Rgb(114, 139, 151)
        } else {
            Color::Rgb(151, 180, 180)
        };
        fill(ctx, 0., 0., 100., 100., sky);
        let orb = if day {
            Color::Rgb(238, 206, 139)
        } else {
            Color::Rgb(222, 224, 204)
        };
        disk(ctx, 23., 76., 10., orb);
        if !day {
            disk(ctx, 27., 80., 9., sky);
            for (i, (x, y)) in [(48., 89.), (79., 84.), (62., 71.), (88., 63.), (12., 91.)]
                .iter()
                .enumerate()
            {
                let color = if (t + i as f64).sin() > 0. {
                    Color::Rgb(205, 214, 218)
                } else {
                    Color::Rgb(135, 152, 172)
                };
                fill(ctx, *x, *y, 1., 1., color);
            }
        }
        if mode != "Sunny" && mode != "Unknown" {
            let speed = if mode == "Windy" { 0.8 } else { 0.18 };
            for (x, y, scale) in [(58., 72., 1.), (29., 58., 0.65)] {
                let drift = (t * speed).sin() * 5.;
                let shade = if day {
                    Color::Rgb(173, 191, 190)
                } else {
                    Color::Rgb(66, 80, 101)
                };
                let color = if storm {
                    Color::Rgb(193, 205, 207)
                } else if day {
                    Color::Rgb(228, 230, 213)
                } else {
                    Color::Rgb(144, 156, 172)
                };
                cloud(ctx, x + drift, y - 2., scale, shade);
                cloud(ctx, x + drift, y + 1., scale, color);
            }
        }
        // Layered hills fill the lower third instead of leaving a flat empty field.
        for x in 0..=100 {
            let xf = x as f64;
            let distant = 26. + (xf / 19.).sin() * 7.;
            let near = 15. + (xf / 15. + 1.7).sin() * 5.;
            let back = if mode == "Snowy" {
                Color::Rgb(180, 197, 200)
            } else if day {
                Color::Rgb(102, 134, 123)
            } else {
                Color::Rgb(49, 72, 76)
            };
            let front = if mode == "Snowy" {
                Color::Rgb(221, 228, 220)
            } else if day {
                Color::Rgb(74, 107, 92)
            } else {
                Color::Rgb(35, 55, 61)
            };
            fill(ctx, xf, 0., 1., distant, back);
            fill(ctx, xf, 0., 1., near, front);
        }
        // A small pine gives the landscape a quiet point of reference.
        let tree = if day {
            Color::Rgb(52, 83, 72)
        } else {
            Color::Rgb(25, 43, 48)
        };
        fill(ctx, 78., 12., 2., 18., tree);
        for (y, width) in [(24., 10.), (31., 8.), (38., 5.)] {
            for n in 0..9 {
                let half = width * (1. - n as f64 / 9.);
                fill(ctx, 79. - half, y + n as f64, half * 2., 1., tree);
            }
        }
        if mode == "Rainy" || mode == "Snowy" {
            for i in 0..28 {
                let speed = if storm { 28. } else { 7. };
                let y = 88. - ((t * speed + i as f64 * 11.) % 76.);
                let x = ((i * 37 % 100) as f64
                    + if storm { 0. } else { (t + i as f64).sin() * 3. })
                .clamp(1., 98.);
                if storm {
                    ctx.draw(&Stroke {
                        x1: x,
                        y1: y,
                        x2: (x - 2.).max(0.),
                        y2: y - 4.,
                        color: Color::Rgb(180, 203, 213),
                    });
                } else {
                    fill(ctx, x, y, 1.3, 1.3, Color::Rgb(235, 238, 227));
                }
            }
        }
        if mode == "Windy" {
            for i in 0..5 {
                let x = (t * 16. + i as f64 * 24.) % 125. - 15.;
                let y = 36. + (i % 3) as f64 * 9.;
                if x + 12. > 0. && x < 100. {
                    ctx.draw(&Stroke {
                        x1: x.max(0.),
                        y1: y,
                        x2: (x + 12.).min(100.),
                        y2: y,
                        color: Color::Rgb(206, 221, 209),
                    });
                }
            }
        }
        for row in 0..art.height {
            for col in 0..art.width {
                let top = raster.average(col as usize * SAMPLES, row as usize * SAMPLES * 2);
                let bottom =
                    raster.average(col as usize * SAMPLES, row as usize * SAMPLES * 2 + SAMPLES);
                f.buffer_mut()[(art.x + col, art.y + row)]
                    .set_symbol("▀")
                    .set_fg(top)
                    .set_bg(bottom);
            }
        }
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
    fn square_viewport_uses_cell_dimensions() {
        for aspect in [1.5, 2.0, 2.4] {
            let area = Rect::new(5, 6, 40, 30);
            let rect = square_viewport(area, aspect);
            assert!((rect.width as f64 - rect.height as f64 * aspect).abs() <= 0.5);
            assert!(rect.right() <= area.right());
            assert!(rect.bottom() <= area.bottom());
        }
    }
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
