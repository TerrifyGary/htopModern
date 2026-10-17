use crate::{block, meter, App, AMBER, CREAM, GREEN};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{Paragraph, Wrap},
    Frame,
};
use sysinfo::System;
fn thermal(app: &App) -> String {
    if let Some(r) = &app.temperature.reading {
        let label = |v: Option<f64>| v.map(|t| format!("{t:.1}°C")).unwrap_or_else(|| "—".into());
        format!("CPU {} · GPU {}", label(r.cpu), label(r.gpu))
    } else {
        app.temperature.status.clone()
    }
}
fn stats(app: &App) -> Vec<Line<'static>> {
    let gb = 1_073_741_824.;
    let sys = &app.sys;
    let load = System::load_average();
    let brand = sys.cpus().first().map(|c| c.brand()).unwrap_or("CPU");
    let memory_pct = if sys.total_memory() > 0 {
        sys.used_memory() as f64 / sys.total_memory() as f64 * 100.
    } else {
        0.
    };
    let mut lines = vec![
        Line::from(app.hardware[0].clone()),
        Line::from(app.hardware[1].clone()),
        Line::from(format!(
            "{brand} · {} physical / {} logical cores",
            sys.physical_core_count().unwrap_or(sys.cpus().len()),
            sys.cpus().len()
        )),
        Line::from(format!(
            "Memory {:.1}/{:.1} GB ({memory_pct:.0}%) · free {:.1} GB",
            sys.used_memory() as f64 / gb,
            sys.total_memory() as f64 / gb,
            sys.free_memory() as f64 / gb
        )),
        Line::from(format!(
            "Swap {:.1}/{:.1} GB",
            sys.used_swap() as f64 / gb,
            sys.total_swap() as f64 / gb
        )),
        Line::from(format!(
            "Processes {} · Load {:.2} {:.2} {:.2}",
            sys.processes().len(),
            load.one,
            load.five,
            load.fifteen
        )),
        Line::from(format!("Uptime {}", crate::uptime_label())),
        Line::styled(thermal(app), Style::default().fg(GREEN)),
    ];
    if let Some(r) = &app.temperature.reading {
        if let (Some(cpu), Some(gpu)) = (r.cpu_power, r.gpu_power) {
            lines.push(Line::from(format!(
                "CPU {cpu:.1} W · GPU {gpu:.1} W · macmon"
            )));
        }
    }
    lines
}
fn processes(f: &mut Frame, area: Rect, app: &App, boxed: bool) {
    let inner = if boxed {
        let b = block(" TOP 3 PROCESSES · CPU ", false);
        let inner = b.inner(area);
        f.render_widget(b, area);
        inner
    } else {
        area
    };
    if inner.height == 0 {
        return;
    }
    let name_width = usize::from(inner.width).saturating_sub(33).clamp(5, 40);
    let mut lines = vec![Line::styled(
        format!(
            " # {:>7} {:<name_width$} {:>7} {:>9}",
            "PID", "PROCESS", "CPU", "MEMORY"
        ),
        Style::default().fg(AMBER),
    )
    .centered()];
    let mut processes: Vec<_> = app.sys.processes().values().collect();
    processes.sort_by(|a, b| {
        b.cpu_usage()
            .total_cmp(&a.cpu_usage())
            .then_with(|| a.pid().cmp(&b.pid()))
    });
    for (i, p) in processes.into_iter().take(3).enumerate() {
        let mut name = String::new();
        for c in p.name().to_string_lossy().chars() {
            let mut next = name.clone();
            next.push(c);
            if Line::from(next.as_str()).width() > name_width {
                break;
            }
            name = next;
        }
        let padding = " ".repeat(name_width.saturating_sub(Line::from(name.as_str()).width()));
        lines.push(
            Line::styled(
                format!(
                    " {} {:>7} {}{} {:>6.1}% {:>7.1}MB",
                    i + 1,
                    p.pid().as_u32(),
                    name,
                    padding,
                    p.cpu_usage(),
                    p.memory() as f64 / 1_048_576.
                ),
                Style::default()
                    .fg(if i == 0 { AMBER } else { CREAM })
                    .add_modifier(Modifier::BOLD),
            )
            .centered(),
        );
    }
    if inner.height >= 7 {
        lines.push(Line::from(""));
        lines.push(Line::from("Ranked by CPU · one core = 100%").centered());
        lines.push(Line::from("Read-only · no process controls").centered());
    }
    f.render_widget(Paragraph::new(lines), inner);
}
pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let b = block(" COMPUTER ", false);
    let inner = b.inner(area);
    f.render_widget(b, area);
    if inner.width < 25 || inner.height < 2 {
        return;
    }
    let cols = (usize::from(inner.width) / 27).max(1);
    let cpus = app.sys.cpus();
    let rows = cpus.len().div_ceil(cols);
    let cpu_height = rows
        .min(usize::from(inner.height).saturating_sub(if inner.width >= 105 { 11 } else { 8 }))
        .max(1) as u16;
    let mut cpu_lines = Vec::new();
    for row in 0..cpu_height as usize {
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
        cpu_lines.push(Line::styled(text, Style::default().fg(GREEN)));
    }
    f.render_widget(
        Paragraph::new(cpu_lines),
        Rect::new(inner.x, inner.y, inner.width, cpu_height.min(inner.height)),
    );
    let divider_y = inner.y + (cpu_height + 1).min(inner.height.saturating_sub(1));
    f.render_widget(
        Paragraph::new(crate::media_layout::divider(inner.width)),
        Rect::new(inner.x, divider_y, inner.width, 1),
    );
    let start = (divider_y + 2).min(inner.bottom());
    let lower = Rect::new(inner.x, start, inner.width, inner.bottom() - start);
    if inner.width >= 105 {
        let chunks = Layout::horizontal([
            Constraint::Percentage(48),
            Constraint::Length(1),
            Constraint::Percentage(52),
        ])
        .split(lower);
        let b = block(" SYSTEM & HARDWARE ", false);
        let details = b.inner(chunks[0]);
        f.render_widget(b, chunks[0]);
        f.render_widget(
            Paragraph::new(stats(app)).wrap(Wrap { trim: false }),
            details,
        );
        processes(f, chunks[2], app, true);
    } else {
        let details_height = lower.height.saturating_sub(4).min(5);
        let chunks =
            Layout::vertical([Constraint::Length(details_height), Constraint::Min(0)]).split(lower);
        let all = stats(app);
        let compact = vec![
            all[3].clone(),
            all[5].clone(),
            all[6].clone(),
            all[7].clone(),
        ];
        f.render_widget(Paragraph::new(compact), chunks[0]);
        processes(f, chunks[1], app, false);
    }
}
