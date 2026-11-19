//! Simple vector icons drawn with shapes, so they look the same on every system.

use eframe::egui::{self, pos2, vec2, Color32, Painter, Pos2, Rect, Shape, Stroke};

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    Play,
    Pause,
    Stop,
    Prev,
    Next,
    Rewind,
    Forward,
    Volume,
    Mute,
    Fullscreen,
    Eject,
    Disc,
    Film,
    Music,
    Photo,
    List,
    Gear,
    Home,
    Subtitles,
    Audio,
    Chapters,
    Camera,
    Folder,
    File,
    Repeat,
    Shuffle,
    Plus,
    Info,
    Computer,
    More,
    Drive,
    Download,
    Video,
}

fn tri(p: &Painter, pts: [Pos2; 3], c: Color32) {
    p.add(Shape::convex_polygon(pts.to_vec(), c, Stroke::NONE));
}

pub fn paint(p: &Painter, icon: Icon, rect: Rect, c: Color32) {
    let r = rect.shrink(rect.width() * 0.18);
    let ctr = r.center();
    let w = r.width();
    let h = r.height();
    let st = Stroke::new((w * 0.09).max(1.4), c);
    match icon {
        Icon::Play => tri(p, [pos2(r.left() + w * 0.18, r.top()), pos2(r.right(), ctr.y), pos2(r.left() + w * 0.18, r.bottom())], c),
        Icon::Pause => {
            let bw = w * 0.28;
            p.rect_filled(Rect::from_min_size(pos2(r.left() + w * 0.1, r.top()), vec2(bw, h)), 1.5, c);
            p.rect_filled(Rect::from_min_size(pos2(r.right() - w * 0.1 - bw, r.top()), vec2(bw, h)), 1.5, c);
        }
        Icon::Stop => {
            p.rect_filled(r.shrink(w * 0.08), 2.0, c);
        }
        Icon::Prev | Icon::Next => {
            let flip = icon == Icon::Prev;
            let x = |f: f32| if flip { r.right() - w * f } else { r.left() + w * f };
            tri(p, [pos2(x(0.0), r.top()), pos2(x(0.75), ctr.y), pos2(x(0.0), r.bottom())], c);
            let bar = Rect::from_x_y_ranges(
                if flip { (r.left())..=(r.left() + w * 0.18) } else { (r.right() - w * 0.18)..=r.right() },
                r.top()..=r.bottom(),
            );
            p.rect_filled(bar, 1.0, c);
        }
        Icon::Rewind | Icon::Forward => {
            let flip = icon == Icon::Rewind;
            let x = |f: f32| if flip { r.right() - w * f } else { r.left() + w * f };
            let (t, b) = (r.top() + h * 0.1, r.bottom() - h * 0.1);
            tri(p, [pos2(x(0.0), t), pos2(x(0.5), ctr.y), pos2(x(0.0), b)], c);
            tri(p, [pos2(x(0.5), t), pos2(x(1.0), ctr.y), pos2(x(0.5), b)], c);
        }
        Icon::Volume | Icon::Mute => {
            let body = Rect::from_min_max(pos2(r.left(), ctr.y - h * 0.18), pos2(r.left() + w * 0.25, ctr.y + h * 0.18));
            p.rect_filled(body, 1.0, c);
            p.add(Shape::convex_polygon(
                vec![
                    pos2(r.left() + w * 0.2, ctr.y - h * 0.18),
                    pos2(r.left() + w * 0.55, r.top()),
                    pos2(r.left() + w * 0.55, r.bottom()),
                    pos2(r.left() + w * 0.2, ctr.y + h * 0.18),
                ],
                c,
                Stroke::NONE,
            ));
            if icon == Icon::Mute {
                let a = pos2(r.left() + w * 0.68, ctr.y - h * 0.2);
                let b = pos2(r.right(), ctr.y + h * 0.2);
                p.line_segment([a, b], st);
                p.line_segment([pos2(a.x, b.y), pos2(b.x, a.y)], st);
            } else {
                for (i, rad) in [0.22f32, 0.4].iter().enumerate() {
                    let n = 10;
                    let o = pos2(r.left() + w * 0.55, ctr.y);
                    let pts: Vec<Pos2> = (0..=n)
                        .map(|k| {
                            let a = -0.9 + 1.8 * k as f32 / n as f32;
                            pos2(o.x + rad * w * a.cos() + w * 0.06 * i as f32, o.y + rad * w * a.sin())
                        })
                        .collect();
                    p.add(Shape::line(pts, st));
                }
            }
        }
        Icon::Fullscreen => {
            let l = w * 0.32;
            for (cx, cy, dx, dy) in [
                (r.left(), r.top(), 1.0, 1.0),
                (r.right(), r.top(), -1.0, 1.0),
                (r.left(), r.bottom(), 1.0, -1.0),
                (r.right(), r.bottom(), -1.0, -1.0),
            ] {
                p.line_segment([pos2(cx, cy), pos2(cx + dx * l, cy)], st);
                p.line_segment([pos2(cx, cy), pos2(cx, cy + dy * l)], st);
            }
        }
        Icon::Eject => {
            tri(p, [pos2(ctr.x, r.top()), pos2(r.right(), r.top() + h * 0.6), pos2(r.left(), r.top() + h * 0.6)], c);
            p.rect_filled(Rect::from_min_max(pos2(r.left(), r.bottom() - h * 0.22), r.max), 1.0, c);
        }
        Icon::Disc => {
            p.circle_stroke(ctr, w * 0.5, st);
            p.circle_filled(ctr, w * 0.14, c);
            p.add(Shape::line(
                (0..=8).map(|k| {
                    let a = -2.4 + 0.9 * k as f32 / 8.0;
                    pos2(ctr.x + w * 0.33 * a.cos(), ctr.y + w * 0.33 * a.sin())
                }).collect(),
                Stroke::new(st.width * 0.8, c),
            ));
        }
        Icon::Film => {
            p.rect_stroke(r, 2.0, st, egui::StrokeKind::Middle);
            let sw = w * 0.2;
            for x in [r.left() + sw, r.right() - sw] {
                p.line_segment([pos2(x, r.top()), pos2(x, r.bottom())], st);
            }
            for i in 0..3 {
                let y = r.top() + h * (0.25 + 0.25 * i as f32);
                p.line_segment([pos2(r.left(), y), pos2(r.left() + sw, y)], Stroke::new(st.width * 0.7, c));
                p.line_segment([pos2(r.right() - sw, y), pos2(r.right(), y)], Stroke::new(st.width * 0.7, c));
            }
        }
        Icon::Music => {
            p.circle_filled(pos2(r.left() + w * 0.22, r.bottom() - h * 0.16), w * 0.18, c);
            p.circle_filled(pos2(r.right() - w * 0.16, r.bottom() - h * 0.26), w * 0.18, c);
            p.line_segment([pos2(r.left() + w * 0.38, r.bottom() - h * 0.16), pos2(r.left() + w * 0.38, r.top() + h * 0.1)], st);
            p.line_segment([pos2(r.right(), r.bottom() - h * 0.26), pos2(r.right(), r.top())], st);
            p.add(Shape::convex_polygon(
                vec![
                    pos2(r.left() + w * 0.38, r.top() + h * 0.1),
                    pos2(r.right(), r.top()),
                    pos2(r.right(), r.top() + h * 0.2),
                    pos2(r.left() + w * 0.38, r.top() + h * 0.3),
                ],
                c,
                Stroke::NONE,
            ));
        }
        Icon::Photo => {
            p.rect_stroke(r, 2.0, st, egui::StrokeKind::Middle);
            tri(p, [pos2(r.left() + w * 0.08, r.bottom() - h * 0.1), pos2(r.left() + w * 0.4, r.top() + h * 0.4), pos2(r.left() + w * 0.72, r.bottom() - h * 0.1)], c);
            tri(p, [pos2(r.left() + w * 0.5, r.bottom() - h * 0.1), pos2(r.left() + w * 0.7, r.top() + h * 0.55), pos2(r.right() - w * 0.06, r.bottom() - h * 0.1)], c);
            p.circle_filled(pos2(r.right() - w * 0.25, r.top() + h * 0.27), w * 0.09, c);
        }
        Icon::List | Icon::Chapters => {
            for i in 0..4 {
                let y = r.top() + h * (0.1 + 0.27 * i as f32);
                if icon == Icon::Chapters {
                    p.circle_filled(pos2(r.left() + w * 0.06, y), w * 0.07, c);
                    p.line_segment([pos2(r.left() + w * 0.26, y), pos2(r.right(), y)], st);
                } else {
                    p.line_segment([pos2(r.left(), y), pos2(r.right(), y)], st);
                }
            }
        }
        Icon::Gear => {
            let n = 8;
            for k in 0..n {
                let a = std::f32::consts::TAU * k as f32 / n as f32;
                let d = vec2(a.cos(), a.sin());
                p.line_segment([ctr + d * w * 0.3, ctr + d * w * 0.52], Stroke::new(w * 0.16, c));
            }
            p.circle_filled(ctr, w * 0.36, c);
            p.circle_filled(ctr, w * 0.14, Color32::from_rgb(24, 26, 32));
        }
        Icon::Home => {
            tri(p, [pos2(ctr.x, r.top()), pos2(r.right() + w * 0.05, ctr.y - h * 0.02), pos2(r.left() - w * 0.05, ctr.y - h * 0.02)], c);
            p.rect_filled(Rect::from_min_max(pos2(r.left() + w * 0.14, ctr.y - h * 0.05), pos2(r.right() - w * 0.14, r.bottom())), 1.0, c);
        }
        Icon::Subtitles => {
            p.rect_stroke(r.shrink2(vec2(0.0, h * 0.1)), 3.0, st, egui::StrokeKind::Middle);
            let y1 = ctr.y + h * 0.02;
            let y2 = ctr.y + h * 0.2;
            let s2 = Stroke::new(st.width * 0.9, c);
            p.line_segment([pos2(r.left() + w * 0.18, y1), pos2(r.left() + w * 0.45, y1)], s2);
            p.line_segment([pos2(r.left() + w * 0.55, y1), pos2(r.right() - w * 0.18, y1)], s2);
            p.line_segment([pos2(r.left() + w * 0.18, y2), pos2(r.right() - w * 0.3, y2)], s2);
        }
        Icon::Audio => {
            let bars = [0.4f32, 0.8, 1.0, 0.6, 0.3];
            let bw = w / (bars.len() as f32 * 1.6);
            for (i, b) in bars.iter().enumerate() {
                let x = r.left() + i as f32 * bw * 1.6 + bw * 0.3;
                let bh = h * b;
                p.rect_filled(Rect::from_min_size(pos2(x, ctr.y - bh / 2.0), vec2(bw, bh)), 1.0, c);
            }
        }
        Icon::Camera => {
            let body = Rect::from_min_max(pos2(r.left(), r.top() + h * 0.22), r.max);
            p.rect_stroke(body, 3.0, st, egui::StrokeKind::Middle);
            p.circle_stroke(body.center(), w * 0.2, st);
            p.rect_filled(Rect::from_min_size(pos2(r.left() + w * 0.3, r.top() + h * 0.05), vec2(w * 0.3, h * 0.18)), 1.0, c);
        }
        Icon::Folder => {
            let body = Rect::from_min_max(pos2(r.left(), r.top() + h * 0.2), r.max);
            p.rect_filled(Rect::from_min_size(pos2(r.left(), r.top() + h * 0.08), vec2(w * 0.42, h * 0.2)), 2.0, c);
            p.rect_filled(body, 2.0, c);
        }
        Icon::File => {
            let body = r.shrink2(vec2(w * 0.12, 0.0));
            p.rect_stroke(body, 2.0, st, egui::StrokeKind::Middle);
            for i in 0..3 {
                let y = body.top() + h * (0.3 + 0.2 * i as f32);
                p.line_segment([pos2(body.left() + w * 0.15, y), pos2(body.right() - w * 0.15, y)], Stroke::new(st.width * 0.7, c));
            }
        }
        Icon::Repeat => {
            let rr = r.shrink2(vec2(0.0, h * 0.15));
            p.rect_stroke(rr, w * 0.2, st, egui::StrokeKind::Middle);
            tri(p, [pos2(ctr.x + w * 0.05, rr.top() - h * 0.15), pos2(ctr.x + w * 0.3, rr.top()), pos2(ctr.x + w * 0.05, rr.top() + h * 0.15)], c);
        }
        Icon::Shuffle => {
            p.line_segment([pos2(r.left(), r.top() + h * 0.2), pos2(r.right() - w * 0.1, r.bottom() - h * 0.2)], st);
            p.line_segment([pos2(r.left(), r.bottom() - h * 0.2), pos2(r.right() - w * 0.1, r.top() + h * 0.2)], st);
            tri(p, [pos2(r.right(), r.top() + h * 0.2), pos2(r.right() - w * 0.25, r.top()), pos2(r.right() - w * 0.25, r.top() + h * 0.4)], c);
            tri(p, [pos2(r.right(), r.bottom() - h * 0.2), pos2(r.right() - w * 0.25, r.bottom()), pos2(r.right() - w * 0.25, r.bottom() - h * 0.4)], c);
        }
        Icon::Plus => {
            p.line_segment([pos2(ctr.x, r.top()), pos2(ctr.x, r.bottom())], st);
            p.line_segment([pos2(r.left(), ctr.y), pos2(r.right(), ctr.y)], st);
        }
        Icon::Computer => {
            let screen = Rect::from_min_max(r.min, pos2(r.right(), r.top() + h * 0.7));
            p.rect_stroke(screen, 1.5, st, egui::StrokeKind::Middle);
            p.rect_filled(Rect::from_min_max(pos2(r.left() - w * 0.08, r.bottom() - h * 0.14), pos2(r.right() + w * 0.08, r.bottom())), 1.0, c);
        }
        Icon::More => {
            for k in [-1.0f32, 0.0, 1.0] {
                p.circle_filled(pos2(ctr.x + k * w * 0.36, ctr.y), w * 0.1, c);
            }
        }
        Icon::Drive => {
            let body = Rect::from_min_max(pos2(r.left(), r.top() + h * 0.3), pos2(r.right(), r.bottom() - h * 0.1));
            p.add(Shape::convex_polygon(
                vec![pos2(r.left() + w * 0.12, r.top() + h * 0.05), pos2(r.right() - w * 0.12, r.top() + h * 0.05), body.right_top(), body.left_top()],
                c,
                Stroke::NONE,
            ));
            p.rect_filled(body, 1.5, c);
            p.line_segment([pos2(body.left(), body.top() + 1.0), pos2(body.right(), body.top() + 1.0)], Stroke::new(1.5, Color32::from_rgb(40, 40, 40)));
            p.circle_filled(pos2(body.right() - w * 0.16, body.center().y + 1.0), w * 0.06, Color32::from_rgb(40, 40, 40));
        }
        Icon::Download => {
            p.rect_filled(Rect::from_center_size(pos2(ctr.x, r.top() + h * 0.25), vec2(w * 0.22, h * 0.45)), 0.0, c);
            tri(p, [pos2(r.left() + w * 0.15, r.top() + h * 0.45), pos2(r.right() - w * 0.15, r.top() + h * 0.45), pos2(ctr.x, r.top() + h * 0.8)], c);
            p.rect_filled(Rect::from_min_max(pos2(r.left() + w * 0.08, r.bottom() - h * 0.08), pos2(r.right() - w * 0.08, r.bottom())), 0.0, c);
        }
        Icon::Video => {
            let body = Rect::from_min_max(pos2(r.left(), r.top() + h * 0.2), pos2(r.left() + w * 0.66, r.bottom() - h * 0.2));
            p.rect_filled(body, 2.0, c);
            tri(p, [pos2(r.left() + w * 0.62, ctr.y), pos2(r.right(), r.top() + h * 0.22), pos2(r.right(), r.bottom() - h * 0.22)], c);
        }
        Icon::Info => {
            p.circle_stroke(ctr, w * 0.5, st);
            p.circle_filled(pos2(ctr.x, ctr.y - h * 0.22), w * 0.07, c);
            p.line_segment([pos2(ctr.x, ctr.y - h * 0.05), pos2(ctr.x, ctr.y + h * 0.3)], st);
        }
    }
}

/// A round or square icon button. Returns the click response.
pub fn button(ui: &mut egui::Ui, icon: Icon, size: f32, tip: &str, active: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), egui::Sense::click());
    let hovered = resp.hovered();
    let col = if active {
        crate::app::ACCENT
    } else if hovered {
        Color32::WHITE
    } else {
        Color32::from_rgb(200, 205, 215)
    };
    if hovered {
        ui.painter().circle_filled(rect.center(), size * 0.5, Color32::from_white_alpha(18));
    }
    paint(ui.painter(), icon, rect.shrink(size * 0.12), col);
    resp.on_hover_text(tip)
}

/// The large circular play/pause button.
pub fn big_button(ui: &mut egui::Ui, icon: Icon, size: f32, tip: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), egui::Sense::click());
    let col = if resp.hovered() { crate::app::ACCENT_HOVER } else { Color32::WHITE };
    if resp.hovered() {
        ui.painter().circle_filled(rect.center(), size * 0.5, Color32::from_white_alpha(14));
    }
    paint(ui.painter(), icon, rect.shrink(size * 0.12), col);
    resp.on_hover_text(tip)
}
