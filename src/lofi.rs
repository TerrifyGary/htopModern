use crate::{block, App, AMBER, CREAM};
use chrono::{Local, Timelike};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    symbols::Marker,
    text::{Line, Span},
    widgets::{
        canvas::{Canvas, Circle, Line as Stroke, Rectangle},
        Paragraph,
    },
    Frame,
};
fn fill_rect(ctx: &mut ratatui::widgets::canvas::Context<'_>, rect: Rectangle) {
    let mut y = rect.y;
    while y <= rect.y + rect.height {
        ctx.draw(&Stroke {
            x1: rect.x,
            y1: y,
            x2: rect.x + rect.width,
            y2: y,
            color: rect.color,
        });
        y += 0.5;
    }
}
const NAMES: [&str; 3] = ["Fireplace", "Beach", "Coffee Shop"];
pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let b = block(" LOFI · L ", app.panel == 0);
    let inner = b.inner(area);
    f.render_widget(b, area);
    if inner.height < 4 {
        f.render_widget(Paragraph::new(NAMES[app.scene]), inner);
        return;
    }
    let clock_height = if inner.height >= 8 { 4 } else { 0 };
    let chunks = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(3),
        Constraint::Length(clock_height),
    ])
    .split(inner);
    let time = app.started.elapsed().as_secs_f64();
    let canvas = Canvas::default()
        .marker(Marker::HalfBlock)
        .x_bounds([0., 100.])
        .y_bounds([0., 70.])
        .paint(|ctx| match app.scene {
            0 => {
                fill_rect(
                    ctx,
                    Rectangle {
                        x: 0.,
                        y: 0.,
                        width: 100.,
                        height: 70.,
                        color: Color::Rgb(53, 45, 41),
                    },
                );
                fill_rect(
                    ctx,
                    Rectangle {
                        x: 13.,
                        y: 6.,
                        width: 74.,
                        height: 58.,
                        color: Color::Rgb(141, 112, 96),
                    },
                );
                fill_rect(
                    ctx,
                    Rectangle {
                        x: 20.,
                        y: 13.,
                        width: 60.,
                        height: 43.,
                        color: Color::Rgb(35, 30, 27),
                    },
                );
                fill_rect(
                    ctx,
                    Rectangle {
                        x: 28.,
                        y: 16.,
                        width: 45.,
                        height: 5.,
                        color: Color::Rgb(117, 82, 58),
                    },
                );
                for (x, base, offset, color) in [
                    (37., 22., 0., Color::Rgb(189, 112, 72)),
                    (51., 31., 1., Color::Rgb(221, 165, 87)),
                    (60., 20., 2., Color::Rgb(240, 213, 154)),
                ] {
                    let height = base + (time * 6. + offset).sin() * 4.;
                    for y in 0..height as usize {
                        let width = 10. * (1. - y as f64 / height);
                        let shift = (time * 5. + y as f64 / 7. + offset).sin() * 2.;
                        ctx.draw(&Stroke {
                            x1: x - width + shift,
                            y1: 21. + y as f64,
                            x2: x + width + shift,
                            y2: 21. + y as f64,
                            color,
                        });
                    }
                }
            }
            1 => {
                fill_rect(
                    ctx,
                    Rectangle {
                        x: 0.,
                        y: 0.,
                        width: 100.,
                        height: 70.,
                        color: Color::Rgb(152, 185, 187),
                    },
                );
                fill_rect(
                    ctx,
                    Rectangle {
                        x: 0.,
                        y: 0.,
                        width: 100.,
                        height: 38.,
                        color: Color::Rgb(86, 127, 135),
                    },
                );
                fill_rect(
                    ctx,
                    Rectangle {
                        x: 0.,
                        y: 0.,
                        width: 100.,
                        height: 12.,
                        color: Color::Rgb(195, 172, 129),
                    },
                );
                ctx.draw(&Circle {
                    x: 79.,
                    y: 56.,
                    radius: 7.,
                    color: Color::Rgb(234, 212, 160),
                });
                for row in [18., 27., 35.] {
                    for x in 0..100 {
                        let y = row + (x as f64 / 10. + time * 2.).sin() * 1.5;
                        ctx.draw(&Stroke {
                            x1: x as f64,
                            y1: y,
                            x2: x as f64 + 1.,
                            y2: y,
                            color: Color::Rgb(198, 220, 209),
                        });
                    }
                }
                ctx.draw(&Stroke {
                    x1: 23.,
                    y1: 8.,
                    x2: 29.,
                    y2: 51.,
                    color: Color::Rgb(120, 97, 76),
                });
                for (dx, dy) in [(-19., -10.), (-20., 1.), (-12., 12.), (16., 8.), (22., -5.)] {
                    let sway = time.sin() * 2.;
                    ctx.draw(&Stroke {
                        x1: 29.,
                        y1: 51.,
                        x2: 29. + dx + sway,
                        y2: 51. + dy,
                        color: Color::Rgb(82, 106, 74),
                    });
                }
            }
            _ => {
                fill_rect(
                    ctx,
                    Rectangle {
                        x: 0.,
                        y: 0.,
                        width: 100.,
                        height: 70.,
                        color: Color::Rgb(87, 69, 55),
                    },
                );
                fill_rect(
                    ctx,
                    Rectangle {
                        x: 8.,
                        y: 31.,
                        width: 34.,
                        height: 31.,
                        color: Color::Rgb(165, 139, 102),
                    },
                );
                fill_rect(
                    ctx,
                    Rectangle {
                        x: 11.,
                        y: 34.,
                        width: 28.,
                        height: 25.,
                        color: Color::Rgb(127, 158, 158),
                    },
                );
                ctx.draw(&Stroke {
                    x1: 25.,
                    y1: 34.,
                    x2: 25.,
                    y2: 59.,
                    color: Color::Rgb(165, 139, 102),
                });
                ctx.draw(&Stroke {
                    x1: 72.,
                    y1: 70.,
                    x2: 72.,
                    y2: 59.,
                    color: AMBER,
                });
                fill_rect(
                    ctx,
                    Rectangle {
                        x: 62.,
                        y: 53.,
                        width: 20.,
                        height: 6.,
                        color: AMBER,
                    },
                );
                fill_rect(
                    ctx,
                    Rectangle {
                        x: 7.,
                        y: 12.,
                        width: 86.,
                        height: 5.,
                        color: Color::Rgb(169, 128, 87),
                    },
                );
                fill_rect(
                    ctx,
                    Rectangle {
                        x: 48.,
                        y: 18.,
                        width: 16.,
                        height: 11.,
                        color: Color::Rgb(223, 207, 177),
                    },
                );
                ctx.draw(&Circle {
                    x: 67.,
                    y: 24.,
                    radius: 4.,
                    color: Color::Rgb(223, 207, 177),
                });
                for offset in [0., 0.5] {
                    let phase = (time * 0.45 + offset) % 1.;
                    let x = 54. + offset * 10. + (time * 2.).sin() * 2.;
                    let y = 31. + phase * 15.;
                    ctx.draw(&Stroke {
                        x1: x,
                        y1: y,
                        x2: x + 2.,
                        y2: y + 5.,
                        color: Color::Rgb(165, 153, 132),
                    });
                }
            }
        });
    f.render_widget(canvas, chunks[0]);
    let selected = Style::default().fg(AMBER);
    let plain = Style::default().fg(CREAM);
    let controls = Line::from(vec![
        Span::styled(
            "[⏮]",
            if app.panel == 0 && app.lofi_action == 0 {
                selected
            } else {
                plain
            },
        ),
        Span::raw("  "),
        Span::styled(
            "[⏭]",
            if app.panel == 0 && app.lofi_action == 1 {
                selected
            } else {
                plain
            },
        ),
    ]);
    f.render_widget(
        Paragraph::new(vec![
            Line::from(if clock_height == 0 {
                format!("{} · {}", NAMES[app.scene], Local::now().format("%H:%M"))
            } else {
                NAMES[app.scene].to_string()
            }),
            controls,
            Line::from("←→ select · Enter switch"),
        ]),
        chunks[1],
    );
    let now = Local::now();
    if clock_height > 0 {
        let sections =
            Layout::horizontal([Constraint::Length(12), Constraint::Min(1)]).split(chunks[2]);
        let clock = Canvas::default()
            .marker(Marker::Braille)
            .x_bounds([-1.3, 1.3])
            .y_bounds([-1.3, 1.3])
            .paint(|ctx| {
                ctx.draw(&Circle {
                    x: 0.,
                    y: 0.,
                    radius: 1.,
                    color: AMBER,
                });
                for n in 0..12 {
                    let a = n as f64 * std::f64::consts::TAU / 12.;
                    ctx.draw(&Stroke {
                        x1: a.sin() * 0.9,
                        y1: a.cos() * 0.9,
                        x2: a.sin(),
                        y2: a.cos(),
                        color: AMBER,
                    });
                }
                let seconds = now.second() as f64;
                for (angle, length, color) in [
                    (
                        now.hour() as f64 / 12. + now.minute() as f64 / 720.,
                        0.5,
                        CREAM,
                    ),
                    (now.minute() as f64 / 60. + seconds / 3600., 0.8, CREAM),
                    (seconds / 60., 0.85, Color::Rgb(172, 101, 112)),
                ] {
                    let a = angle * std::f64::consts::TAU;
                    ctx.draw(&Stroke {
                        x1: 0.,
                        y1: 0.,
                        x2: a.sin() * length,
                        y2: a.cos() * length,
                        color,
                    });
                }
            });
        f.render_widget(clock, sections[0]);
        f.render_widget(
            Paragraph::new(format!("\n{}\nDevice time", now.format("%H:%M:%S"))),
            sections[1],
        );
    }
}
