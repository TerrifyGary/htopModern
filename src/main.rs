mod graphics;
mod lofi;
mod music;
mod weather;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    io::{self, IsTerminal},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use sysinfo::{ProcessesToUpdate, System};

const CREAM: Color = Color::Rgb(220, 212, 189);
const AMBER: Color = Color::Rgb(205, 167, 109);
const GREEN: Color = Color::Rgb(165, 185, 121);
const BG: Color = Color::Rgb(32, 33, 31);

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
struct Task {
    text: String,
    done: bool,
}
fn data_path() -> io::Result<PathBuf> {
    if let Some(p) = env::var_os("HTOPMODERN_DATA_DIR") {
        return Ok(PathBuf::from(p).join("todos.json"));
    }
    let home = env::var_os("HOME")
        .ok_or_else(|| io::Error::other("HOME is not set; use HTOPMODERN_DATA_DIR"))?;
    Ok(PathBuf::from(home).join("Library/Application Support/htopmodern/todos.json"))
}
fn load_tasks(path: &Path) -> io::Result<Vec<Task>> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(vec![]),
        Err(e) => Err(e),
    }
}
fn save_tasks(path: &Path, tasks: &[Task]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("json.tmp");
    fs::write(
        &temp,
        serde_json::to_vec_pretty(tasks).map_err(io::Error::other)?,
    )?;
    fs::rename(temp, path)
}
struct App {
    sys: System,
    tasks: Vec<Task>,
    path: PathBuf,
    panel: usize,
    selected: usize,
    action: usize,
    input: Option<String>,
    prefix: bool,
    message: String,
    scene: usize,
    lofi_action: usize,
    started: Instant,
    cell_aspect: f64,
    aspect_override: Option<f64>,
    graphics: std::cell::RefCell<graphics::Graphics>,
    weather: weather::Weather,
    music: music::Music,
}
impl App {
    fn new(path: PathBuf) -> io::Result<Self> {
        let tasks = load_tasks(&path)?;
        let mut app = Self {
            weather: weather::Weather::new(path.with_file_name("weather.json")),
            music: music::Music::new(),
            sys: System::new(),
            tasks,
            path,
            panel: 2,
            selected: 0,
            action: 0,
            input: None,
            prefix: false,
            message: String::new(),
            scene: 0,
            lofi_action: 0,
            started: Instant::now(),
            cell_aspect: cell_aspect(),
            aspect_override: None,
            graphics: std::cell::RefCell::new(graphics::Graphics::new()),
        };
        app.refresh();
        Ok(app)
    }
    fn refresh(&mut self) {
        self.sys.refresh_cpu_all();
        self.sys.refresh_memory();
        self.sys.refresh_processes(ProcessesToUpdate::All, true);
    }
    fn persist(&mut self, proposed: Vec<Task>) {
        match save_tasks(&self.path, &proposed) {
            Ok(()) => {
                self.tasks = proposed;
                self.message = "Saved".into();
            }
            Err(e) => self.message = format!("Could not save: {e}"),
        }
    }
    fn key(&mut self, key: KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return true;
        }
        if let Some(input) = self.weather.input.as_mut() {
            match key.code {
                KeyCode::Esc => self.weather.input = None,
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(c)
                    if !key.modifiers.contains(KeyModifiers::CONTROL)
                        && input.chars().count() < 80 =>
                {
                    input.push(c)
                }
                KeyCode::Enter => {
                    let query = input.trim().to_string();
                    if !query.is_empty() {
                        self.weather.input = None;
                        self.weather.set_query(query);
                    }
                }
                _ => {}
            }
            return false;
        }
        if let Some(input) = self.input.as_mut() {
            match key.code {
                KeyCode::Esc => self.input = None,
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(c)
                    if !key.modifiers.contains(KeyModifiers::CONTROL)
                        && input.chars().count() < 200 =>
                {
                    input.push(c)
                }
                KeyCode::Enter => {
                    let text = input.trim().to_owned();
                    if !text.is_empty() {
                        let mut tasks = self.tasks.clone();
                        tasks.push(Task { text, done: false });
                        let old = self.tasks.len();
                        self.persist(tasks);
                        if self.tasks.len() > old {
                            self.selected = old;
                            self.action = 0;
                            self.input = None;
                        }
                    }
                }
                _ => {}
            }
            return false;
        }
        if self.prefix {
            self.prefix = false;
            if let KeyCode::Char(c) = key.code {
                if let Some(i) = ['l', 'm', 't', 'w']
                    .iter()
                    .position(|&v| v == c.to_ascii_lowercase())
                {
                    self.panel = i;
                }
            }
            return false;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('b') {
            self.prefix = true;
            return false;
        }
        match key.code {
            KeyCode::Char('q') => return true,
            KeyCode::Char('l') if self.panel == 1 => {
                self.music.show_lyrics = !self.music.show_lyrics
            }
            KeyCode::Up if self.panel == 1 => self.music.scroll_lyrics(false),
            KeyCode::Down if self.panel == 1 => self.music.scroll_lyrics(true),
            KeyCode::Left if self.panel == 1 => self.music.selected = (self.music.selected + 2) % 3,
            KeyCode::Right if self.panel == 1 => {
                self.music.selected = (self.music.selected + 1) % 3
            }
            KeyCode::Enter if self.panel == 1 => self.music.activate(),
            KeyCode::Char('r') if self.panel == 1 => self.music.request(music::Action::Read),
            KeyCode::Enter | KeyCode::Char('e') if self.panel == 3 => {
                self.weather.input = Some(self.weather.query.clone())
            }
            KeyCode::Char('r') if self.panel == 3 => self.weather.request(),
            KeyCode::Left | KeyCode::Right if self.panel == 0 => {
                self.lofi_action = 1 - self.lofi_action
            }
            KeyCode::Enter if self.panel == 0 => {
                self.scene = (self.scene + if self.lofi_action == 0 { 2 } else { 1 }) % 3
            }
            KeyCode::Tab => self.panel = (self.panel + 1) % 4,
            KeyCode::BackTab => self.panel = (self.panel + 3) % 4,
            KeyCode::Char('a') if self.panel == 2 => self.input = Some(String::new()),
            KeyCode::Up if self.panel == 2 => self.selected = self.selected.saturating_sub(1),
            KeyCode::Down if self.panel == 2 => {
                self.selected = (self.selected + 1).min(self.tasks.len())
            }
            KeyCode::Left | KeyCode::Right if self.panel == 2 => self.action = 1 - self.action,
            KeyCode::Enter if self.panel == 2 => {
                if self.selected >= self.tasks.len() {
                    self.input = Some(String::new());
                } else {
                    let mut tasks = self.tasks.clone();
                    if self.action == 0 {
                        tasks[self.selected].done = !tasks[self.selected].done;
                    } else {
                        tasks.remove(self.selected);
                    }
                    self.persist(tasks);
                    self.selected = self.selected.min(self.tasks.len());
                }
            }
            _ => {}
        }
        false
    }
}
fn block(title: &str, active: bool) -> Block<'_> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if active {
            AMBER
        } else {
            Color::Rgb(107, 101, 86)
        }))
}
fn meter(value: f32, width: usize) -> String {
    let filled = ((value.clamp(0., 100.) / 100.) * width as f32).round() as usize;
    format!("{}{}", "┃".repeat(filled), "·".repeat(width - filled))
}
fn computer(f: &mut Frame, area: Rect, app: &App) {
    let b = block(" COMPUTER ", false);
    let inner = b.inner(area);
    f.render_widget(b, area);
    if inner.width < 25 || inner.height < 2 {
        return;
    }
    let cols = (usize::from(inner.width) / 27).max(1);
    let cpus = app.sys.cpus();
    let rows = cpus.len().div_ceil(cols);
    let mut lines = Vec::new();
    for row in 0..rows.min(usize::from(inner.height).saturating_sub(7)) {
        let mut text = String::new();
        for col in 0..cols {
            let i = col * rows + row;
            if let Some(cpu) = cpus.get(i) {
                text.push_str(&format!(
                    "{i:>2} [{}] {:>5.1}%  ",
                    meter(cpu.cpu_usage(), 12),
                    cpu.cpu_usage()
                ));
            }
        }
        lines.push(Line::styled(text, Style::default().fg(GREEN)));
    }
    let separator = || {
        Line::styled(
            "- ".repeat(usize::from(inner.width).div_ceil(2))
                .chars()
                .take(usize::from(inner.width))
                .collect::<String>(),
            Style::default().fg(Color::Rgb(107, 101, 86)),
        )
    };
    lines.push(separator());
    let gb = 1_073_741_824.;
    lines.push(Line::from(format!(
        "Mem {:.1}/{:.1} GB   Swap {:.1}/{:.1} GB",
        app.sys.used_memory() as f64 / gb,
        app.sys.total_memory() as f64 / gb,
        app.sys.used_swap() as f64 / gb,
        app.sys.total_swap() as f64 / gb
    )));
    let load = System::load_average();

    lines.push(Line::from(format!(
        "Processes {}   Load {:.2} {:.2} {:.2}",
        app.sys.processes().len(),
        load.one,
        load.five,
        load.fifteen
    )));
    lines.push(Line::from(format!(
        "Uptime {}   CPU temp: unavailable",
        uptime_label()
    )));
    lines.push(separator());
    lines.push(Line::styled(
        "PROCESS                         CPU       MEMORY",
        Style::default().fg(AMBER),
    ));
    let mut processes: Vec<_> = app.sys.processes().values().collect();
    processes.sort_by(|a, b| b.cpu_usage().total_cmp(&a.cpu_usage()));
    for p in processes
        .into_iter()
        .take(usize::from(inner.height).saturating_sub(lines.len()))
    {
        let name: String = p.name().to_string_lossy().chars().take(28).collect();
        lines.push(Line::from(format!(
            "{name:<28} {:>6.1}%   {:>7.1} MB",
            p.cpu_usage(),
            p.memory() as f64 / 1_048_576.
        )));
    }
    f.render_widget(Paragraph::new(lines), inner);
}
fn todos(f: &mut Frame, area: Rect, app: &App) {
    let b = block(" TODO · T ", app.panel == 2);
    let inner = b.inner(area);
    f.render_widget(b, area);
    let mut lines = Vec::new();
    let capacity = usize::from(inner.height).saturating_sub(4).max(1);
    let start = app.selected.saturating_sub(capacity - 1);
    for (i, t) in app.tasks.iter().enumerate().skip(start).take(capacity) {
        let active = app.panel == 2 && app.selected == i;
        let check = if t.done { "☑" } else { "□" };
        let left = if active && app.action == 0 {
            format!("[{check}]")
        } else {
            format!(" {check} ")
        };
        let del = if active && app.action == 1 {
            "[Delete]"
        } else {
            " Delete "
        };
        let max = usize::from(inner.width).saturating_sub(15);
        let title: String = t.text.chars().take(max).collect();
        lines.push(Line::styled(
            format!("{left} {title} {del}"),
            Style::default().fg(if active { AMBER } else { CREAM }),
        ));
    }
    lines.push(Line::styled(
        "[＋ Add item]",
        Style::default().fg(if app.panel == 2 && app.selected >= app.tasks.len() {
            AMBER
        } else {
            CREAM
        }),
    ));
    lines.push(Line::from("A add · ↑↓ task · ←→ action"));
    lines.push(Line::from("Enter activate"));
    if let Some(input) = &app.input {
        lines.push(Line::styled(
            format!("New: {input}▏"),
            Style::default().fg(AMBER),
        ));
    }
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}
fn ui(f: &mut Frame, app: &App) {
    let area = f.area();
    f.render_widget(
        Block::default().style(Style::default().bg(BG).fg(CREAM)),
        area,
    );
    if area.width < 40 || area.height < 14 {
        f.render_widget(
            Paragraph::new("Please enlarge terminal to at least 40×14. Q quits."),
            area,
        );
        return;
    }
    let rows = app
        .sys
        .cpus()
        .len()
        .div_ceil((usize::from(area.width) / 27).max(1));
    let top = (rows + 11).min(usize::from(area.height) / 2).max(6) as u16;
    let parts = Layout::vertical([
        Constraint::Length(top),
        Constraint::Length(1),
        Constraint::Min(5),
        Constraint::Length(2),
    ])
    .split(area);
    computer(f, parts[0], app);
    if area.width >= 110 {
        let panels = Layout::horizontal([Constraint::Percentage(25); 4]).split(parts[2]);
        for i in 0..4 {
            if i == 0 {
                lofi::draw(f, panels[i], app);
            } else if i == 3 {
                weather::draw(f, panels[i], app);
            } else if i == 2 {
                todos(f, panels[i], app);
            } else {
                music::draw(f, panels[i], app);
            }
        }
    } else if app.panel == 0 {
        lofi::draw(f, parts[2], app);
    } else if app.panel == 3 {
        weather::draw(f, parts[2], app);
    } else if app.panel == 2 {
        todos(f, parts[2], app);
    } else {
        music::draw(f, parts[2], app);
    }

    let status = if app.prefix {
        "Choose L / M / T / W"
    } else {
        "Ctrl+B → L/M/T/W · Tab panel · Q quit"
    };
    f.render_widget(
        Paragraph::new(format!("{status}\n{}", app.message)),
        parts[3],
    );
}
fn uptime_label() -> String {
    if System::boot_time() == 0 {
        return "unavailable".into();
    }
    let seconds = System::uptime();
    format!(
        "{}d {:02}:{:02}:{:02}",
        seconds / 86400,
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60
    )
}
fn cell_aspect() -> f64 {
    if let Ok(value) = env::var("HTOPMODERN_CELL_ASPECT") {
        if let Ok(ratio) = value.parse::<f64>() {
            if (1.0..=4.0).contains(&ratio) {
                return ratio;
            }
        }
    }
    if let Ok(size) = crossterm::terminal::window_size() {
        if size.width > 0 && size.height > 0 && size.columns > 0 && size.rows > 0 {
            let ratio =
                (size.height as f64 / size.rows as f64) / (size.width as f64 / size.columns as f64);
            if (1.0..=4.0).contains(&ratio) {
                return ratio;
            }
        }
    }
    2.3
}
fn main() -> io::Result<()> {
    let args: Vec<_> = env::args().collect();
    if args.get(1).is_some_and(|a| a == "--music-check") {
        let reply = music::query(music::Action::Read).map_err(io::Error::other)?;
        println!("{}", reply.status);
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "--weather-check") {
        let query = args.get(2).map(String::as_str).unwrap_or("Taipei");
        let snapshot = weather::fetch(query).map_err(io::Error::other)?;
        println!(
            "{} · {} · {:.1}°C · {} · Humidity: {} · Rain chance (hour): {}",
            snapshot.location,
            snapshot.condition(),
            snapshot.temperature,
            if snapshot.day { "Day" } else { "Night" },
            weather::percent_label(snapshot.humidity),
            weather::percent_label(snapshot.rain_probability)
        );
        return Ok(());
    }

    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("htopmodern — cozy terminal dashboard\n\nCtrl+B then L/M/T/W: panel · Tab: next panel · Q: quit\nTODO: A adds, ↑↓ selects, ←→ selects checkbox/delete, Enter activates\nEntry: Enter saves, Escape cancels\nHTOPMODERN_DATA_DIR overrides todo storage.\n--cell-aspect 2.3 calibrates square weather scenes for your font.\n--check prints a system snapshot without entering full screen.");
        return Ok(());
    }
    if args.iter().any(|a| a == "--check") {
        let mut sys = System::new();
        sys.refresh_cpu_all();
        std::thread::sleep(Duration::from_millis(300));
        sys.refresh_cpu_all();
        sys.refresh_memory();
        sys.refresh_processes(ProcessesToUpdate::All, true);
        println!(
            "CPUs: {} · Memory: {} MB · Processes: {} · Uptime: {}",
            sys.cpus().len(),
            sys.used_memory() / 1_048_576,
            sys.processes().len(),
            uptime_label()
        );
        return Ok(());
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(io::Error::other(
            "Run in an interactive terminal, or use --check.",
        ));
    }
    let mut app = App::new(data_path()?)?;
    if let Some(index) = args.iter().position(|arg| arg == "--cell-aspect") {
        let ratio = args
            .get(index + 1)
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| (1.0..=4.0).contains(v))
            .ok_or_else(|| io::Error::other("--cell-aspect requires a number between 1 and 4"))?;
        app.cell_aspect = ratio;
        app.aspect_override = Some(ratio);
    }
    app.weather.request();
    app.music.request(music::Action::Read);
    let mut terminal = ratatui::init();
    app.message = app.graphics.borrow_mut().detect();
    let result = (|| {
        let mut refreshed = Instant::now();
        loop {
            app.weather.tick();
            app.music.tick();
            terminal.draw(|f| ui(f, &app))?;
            if event::poll(Duration::from_millis(100))? {
                let event = event::read()?;
                if matches!(event, Event::Resize(_, _)) {
                    app.cell_aspect = app.aspect_override.unwrap_or_else(cell_aspect);
                }
                if let Event::Key(key) = event {
                    if key.kind == KeyEventKind::Press && app.key(key) {
                        break;
                    }
                }
            }
            if refreshed.elapsed() >= Duration::from_secs(1) {
                app.refresh();
                refreshed = Instant::now();
            }
        }
        Ok(())
    })();
    ratatui::restore();
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persistence_and_invalid_data() {
        let dir = env::temp_dir().join(format!("htopmodern-test-{}", std::process::id()));
        let path = dir.join("todos.json");
        let tasks = vec![Task {
            text: "Read 日本語".into(),
            done: true,
        }];
        save_tasks(&path, &tasks).unwrap();
        assert_eq!(load_tasks(&path).unwrap(), tasks);
        fs::write(&path, "broken").unwrap();
        assert!(load_tasks(&path).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn todo_keyboard_lifecycle() {
        let dir = env::temp_dir().join(format!("htopmodern-keys-{}", std::process::id()));
        let path = dir.join("todos.json");
        let mut app = App::new(path.clone()).unwrap();
        let press = |app: &mut App, code| {
            app.key(KeyEvent::new(code, KeyModifiers::NONE));
        };
        press(&mut app, KeyCode::Enter);
        for c in "New task".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        press(&mut app, KeyCode::Enter);
        assert_eq!(load_tasks(&path).unwrap()[0].text, "New task");
        press(&mut app, KeyCode::Enter);
        assert!(load_tasks(&path).unwrap()[0].done);
        press(&mut app, KeyCode::Right);
        press(&mut app, KeyCode::Enter);
        assert!(load_tasks(&path).unwrap().is_empty());
        press(&mut app, KeyCode::Char('a'));
        press(&mut app, KeyCode::Char('x'));
        press(&mut app, KeyCode::Esc);
        assert!(app.input.is_none());
        assert!(app.tasks.is_empty());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn lofi_controls_and_rendering() {
        let mut app = App::new(env::temp_dir().join("htopmodern-lofi-no-file.json")).unwrap();
        app.panel = 0;
        app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.scene, 2);
        app.key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.scene, 0);
        let mut outputs = Vec::new();
        for scene in 0..3 {
            app.scene = scene;
            let mut term =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(40, 20)).unwrap();
            term.draw(|f| lofi::draw(f, f.area(), &app)).unwrap();
            outputs.push(term.backend().buffer().clone());
            for (w, h) in [(1, 1), (15, 4), (25, 9)] {
                let mut tiny =
                    ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, h)).unwrap();
                tiny.draw(|f| lofi::draw(f, f.area(), &app)).unwrap();
            }
        }
        assert_ne!(outputs[0], outputs[1]);
        assert_ne!(outputs[1], outputs[2]);
    }
    #[test]
    fn small_terminal_renders() {
        let path = env::temp_dir().join("htopmodern-render-no-file.json");
        let app = App::new(path).unwrap();
        for (w, h) in [(40, 14), (80, 24), (120, 40), (20, 8)] {
            let backend = ratatui::backend::TestBackend::new(w, h);
            let mut term = ratatui::Terminal::new(backend).unwrap();
            term.draw(|f| ui(f, &app)).unwrap();
        }
    }
}
