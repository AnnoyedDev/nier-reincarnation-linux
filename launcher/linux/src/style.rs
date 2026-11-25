use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui};

use crate::lang::tr;

pub const INK: Color32 = Color32::from_rgb(0xe8, 0xe4, 0xd8);
pub const DIM: Color32 = Color32::from_rgb(0x8f, 0x8a, 0x80);
pub const LINE: Color32 = Color32::from_rgb(0xa9, 0xa5, 0x9b);
pub const DIALOG: Color32 = Color32::from_rgb(0x2b, 0x28, 0x26);
pub const PAGE: Color32 = Color32::from_rgb(0x3c, 0x37, 0x34);
pub const BAR: Color32 = Color32::from_rgb(0xcf, 0xcc, 0xc0);
pub const ON_BAR: Color32 = Color32::from_rgb(0x3a, 0x36, 0x33);
pub const CREAM: Color32 = Color32::from_rgb(0xe9, 0xe6, 0xd9);
pub const STONE: Color32 = Color32::from_rgb(0x8e, 0x8a, 0x82);
pub const ALERT: Color32 = Color32::from_rgb(0xe4, 0xa3, 0xa0);
pub const SOFT: Color32 = Color32::from_rgb(0xa2, 0x9d, 0x93);
pub const FRAME: Color32 = Color32::from_rgb(0x6d, 0x69, 0x63);
pub const EDGE: Color32 = Color32::from_rgb(0x5d, 0x59, 0x53);
pub const INNER: Color32 = Color32::from_rgb(0x4a, 0x46, 0x42);

pub fn font(size: f32) -> FontId {
    FontId::proportional(size)
}

pub fn spaced(ui: &Ui, text: &str, size: f32, color: Color32, spacing: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::default();
    job.append(text, 0.0, egui::TextFormat { font_id: font(size), color, extra_letter_spacing: spacing, ..Default::default() });
    ui.fonts_mut(|f| f.layout_job(job))
}

pub fn wrapped(ui: &Ui, text: &str, size: f32, color: Color32, width: f32) -> std::sync::Arc<egui::Galley> {
    ui.fonts_mut(|f| f.layout(text.to_owned(), font(size), color, width))
}

fn alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a * 255.0) as u8)
}
pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, (a * 255.0) as u8)
}

pub fn lead(ui: &mut Ui, text: &str) {
    lead_colored(ui, text, INK);
}
pub fn lead_colored(ui: &mut Ui, text: &str, color: Color32) {
    if text.is_empty() {
        return;
    }
    let g = wrapped(ui, tr(text), 16.0, color, ui.available_width());
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), g.size().y + 2.0), Sense::hover());
    ui.painter().galley(r.min, g, color);
}

pub fn bar(ui: &mut Ui, text: &str) {
    ui.add_space(4.0);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, BAR);
    ui.painter().text(pos2(r.left() + 18.0, r.center().y), Align2::LEFT_CENTER, tr(text), font(19.0), ON_BAR);
}

fn stud(p: &egui::Painter, center: Pos2) {
    let r = Rect::from_center_size(center, vec2(10.0, 10.0));
    p.rect_filled(r, 0.0, DIALOG);
    p.rect_stroke(r, 0.0, Stroke::new(2.0, LINE), StrokeKind::Inside);
}

fn diamonds(p: &egui::Painter, at: Pos2, toward_right: bool) {
    let d = if toward_right { 1.0 } else { -1.0 };
    let one = |c: Pos2| {
        let s = 7.5;
        let pts = vec![pos2(c.x, c.y - s), pos2(c.x + s, c.y), pos2(c.x, c.y + s), pos2(c.x - s, c.y)];
        p.add(Shape::convex_polygon(pts, DIALOG, Stroke::new(2.0, LINE)));
    };
    one(pos2(at.x - d * 2.0, at.y));
    one(pos2(at.x + d * 5.0, at.y));
}

pub fn dialog<R>(ui: &mut Ui, title: &str, body: impl FnOnce(&mut Ui) -> R) -> R {
    ui.add_space(10.0);
    let outer = ui.available_width();
    let frame = egui::Frame::new().fill(DIALOG).stroke(Stroke::new(2.0, FRAME))
        .inner_margin(egui::Margin { left: 30, right: 30, top: 18, bottom: 26 });
    let mut out = None;
    let resp = egui::Frame::new().outer_margin(egui::Margin::symmetric(8, 0)).show(ui, |ui| {
        ui.set_width(outer - 16.0);
        frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 12.0;
            if !title.is_empty() {
                let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::hover());
                let g = spaced(ui, tr(title), 26.0, INK, 1.0);
                let tw = g.size().x;
                let c = r.center();
                let p = ui.painter();
                p.galley(pos2(c.x - tw / 2.0, c.y - g.size().y / 2.0), g, INK);
                let l1 = c.x - tw / 2.0 - 22.0;
                let l2 = c.x + tw / 2.0 + 22.0;
                p.line_segment([pos2(r.left(), c.y), pos2(l1 - 16.0, c.y)], Stroke::new(2.0, LINE));
                p.line_segment([pos2(l2 + 16.0, c.y), pos2(r.right(), c.y)], Stroke::new(2.0, LINE));
                diamonds(p, pos2(l1 - 10.0, c.y), true);
                diamonds(p, pos2(l2 + 10.0, c.y), false);
            }
            out = Some(body(ui));
        });
    });
    let r = resp.response.rect.shrink2(vec2(8.0, 0.0));
    let p = ui.painter();
    p.rect_stroke(r.shrink(2.0), 0.0, Stroke::new(1.0, INNER), StrokeKind::Inside);
    for c in [r.left_top(), r.right_top(), r.left_bottom(), r.right_bottom()] {
        let o = vec2(if c.x < r.center().x { -4.0 } else { 4.0 }, if c.y < r.center().y { -4.0 } else { 4.0 });
        stud(p, c + o);
    }
    ui.add_space(30.0);
    out.unwrap()
}

pub fn hex(ui: &mut Ui, text: &str, dim: bool, small: bool, enabled: bool) -> Response {
    let text = tr(text);
    let (w, h, size): (f32, f32, f32) = if small { (140.0, 40.0, 17.0) } else { (230.0, 50.0, 21.0) };
    let g = spaced(ui, text, size, Color32::BLACK, 0.0);
    let w = w.max(g.size().x + 90.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), if enabled { Sense::click() } else { Sense::hover() });
    let hover = enabled && resp.hovered();
    let fade = |c: Color32| if enabled { c } else { c.gamma_multiply(0.45) };
    let lift = |c: Color32| if hover { c.lerp_to_gamma(Color32::WHITE, 0.08) } else { c };
    let p = ui.painter();
    let poly = |r: Rect, cut: f32| vec![pos2(r.left() + cut, r.top()), pos2(r.right() - cut, r.top()), pos2(r.right(), r.center().y),
                                         pos2(r.right() - cut, r.bottom()), pos2(r.left() + cut, r.bottom()), pos2(r.left(), r.center().y)];
    let (outer, inner, ink, hatch) = if dim {
        (Color32::from_rgb(0xc9, 0xc5, 0xbb), STONE, Color32::from_rgb(0xf2, 0xef, 0xe6), rgba(255, 255, 255, 0.3))
    } else {
        (Color32::from_rgb(0xf4, 0xf1, 0xe8), CREAM, Color32::from_rgb(0x3b, 0x37, 0x34), rgba(60, 55, 50, 0.45))
    };
    p.add(Shape::convex_polygon(poly(rect, 22.0), fade(lift(outer)), Stroke::NONE));
    let ir = rect.shrink(3.0);
    p.add(Shape::convex_polygon(poly(ir, 20.0), fade(lift(inner)), Stroke::NONE));
    let clip = |x: f32, y: f32, left: bool| {
        let d = if left { x - ir.left() } else { ir.right() - x };
        let half = ir.height() / 2.0;
        (y - ir.center().y).abs() <= half * (d / 20.0).min(1.0)
    };
    for left in [true, false] {
        let mut x0 = 0.0;
        while x0 < 34.0 + ir.height() {
            let mut pts = Vec::new();
            for i in 0..=24 {
                let t = i as f32 / 24.0;
                let y = ir.top() + t * ir.height();
                let dx = x0 - t * ir.height() * 0.577;
                let x = if left { ir.left() + dx } else { ir.right() - dx };
                if (0.0..=34.0).contains(&dx) && clip(x, y, left) {
                    pts.push(pos2(x, y));
                } else if pts.len() > 1 {
                    break;
                } else {
                    pts.clear();
                }
            }
            if pts.len() > 1 {
                p.line(pts, Stroke::new(2.0, fade(hatch)));
            }
            x0 += 6.0;
        }
    }
    let g = spaced(ui, text, size, fade(ink), 0.0);
    ui.painter().galley(rect.center() - g.size() / 2.0, g, fade(ink));
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

pub fn buttons(ui: &mut Ui, list: &[(&str, bool, bool)]) -> Option<usize> {
    let mut clicked = None;
    let widths: Vec<f32> = list.iter().map(|(t, _, _)| 230.0_f32.max(spaced(ui, tr(t), 21.0, Color32::BLACK, 0.0).size().x + 90.0)).collect();
    let total: f32 = widths.iter().sum::<f32>() + 30.0 * (list.len().saturating_sub(1)) as f32;
    ui.add_space(6.0);
    let avail = ui.available_width();
    let mut one = |ui: &mut Ui, i: usize| {
        let (text, dim, enabled) = list[i];
        if hex(ui, text, dim, false, enabled).clicked() {
            clicked = Some(i);
        }
    };
    if total <= avail {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 30.0;
            ui.add_space((avail - total) / 2.0 - 30.0 * 0.0);
            for i in 0..list.len() {
                one(ui, i);
            }
        });
    } else {
        for (i, w) in widths.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.add_space(((avail - w) / 2.0).max(0.0));
                one(ui, i);
            });
        }
    }
    clicked
}

pub fn number(ui: &mut Ui, value: &mut String, hint: &str, width: f32) -> Response {
    let resp = ui.add(egui::TextEdit::singleline(value).frame(egui::Frame::NONE.fill(rgba(0, 0, 0, 0.18))).desired_width(width)
        .font(font(15.0)).text_color(INK).hint_text(egui::RichText::new(hint).color(DIM)).margin(vec2(6.0, 5.0)));
    let color = if resp.has_focus() { INK } else { FRAME };
    ui.painter().line_segment([resp.rect.left_bottom(), resp.rect.right_bottom()], Stroke::new(1.0, color));
    resp
}

pub fn text_area(ui: &mut Ui, value: &mut String, hint: &str) -> Response {
    let resp = ui.add(egui::TextEdit::multiline(value).frame(egui::Frame::NONE.fill(rgba(0, 0, 0, 0.18))).desired_width(ui.available_width())
        .desired_rows(3).font(FontId::monospace(13.0)).text_color(INK).hint_text(egui::RichText::new(hint).color(DIM)).margin(vec2(6.0, 5.0)));
    let color = if resp.has_focus() { INK } else { FRAME };
    ui.painter().line_segment([resp.rect.left_bottom(), resp.rect.right_bottom()], Stroke::new(1.0, color));
    resp
}

fn diamond_shape(c: Pos2, half: f32) -> Vec<Pos2> {
    vec![pos2(c.x, c.y - half), pos2(c.x + half, c.y), pos2(c.x, c.y + half), pos2(c.x - half, c.y)]
}

pub fn choice_item(ui: &mut Ui, label: &str, on: bool, width: f32, faded: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 38.0), Sense::click());
    let p = ui.painter();
    let c = pos2(rect.left() + 4.0 + 19.0, rect.center().y);
    let half = 15.5;
    if on {
        p.add(Shape::convex_polygon(diamond_shape(c, half + 3.0), alpha(INK, 0.18), Stroke::NONE));
        p.add(Shape::convex_polygon(diamond_shape(c, half), DIALOG, Stroke::new(2.0, INK)));
        p.add(Shape::convex_polygon(diamond_shape(c, half - 6.5), INK, Stroke::NONE));
    } else {
        p.add(Shape::convex_polygon(diamond_shape(c, half), Color32::TRANSPARENT, Stroke::new(2.0, Color32::from_rgb(0x7d, 0x79, 0x72))));
    }
    let color = if on { Color32::WHITE } else if resp.hovered() { INK } else { SOFT };
    let color = if faded { color.gamma_multiply(0.35) } else { color };
    p.text(pos2(rect.left() + 4.0 + 38.0 + 14.0, rect.center().y), Align2::LEFT_CENTER, tr(label), font(19.0), color);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn choice_grid(ui: &mut Ui, options: &[(&str, &str)], current: &str) -> Option<String> {
    let w = ui.available_width() - 8.0;
    let cols = ((w + 20.0) / 190.0).floor().max(1.0) as usize;
    let cell = (w - 20.0 * (cols as f32 - 1.0)) / cols as f32;
    let mut picked = None;
    for row in options.chunks(cols) {
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.spacing_mut().item_spacing.x = 20.0;
            for (value, label) in row {
                if choice_item(ui, label, *value == current, cell, false).clicked() {
                    picked = Some(value.to_string());
                }
            }
        });
    }
    picked
}

pub fn tab(ui: &mut Ui, text: &str, on: bool, min_w: f32, h: f32, size: f32, inset: f32) -> Response {
    let text = tr(text);
    let g = spaced(ui, text, size, Color32::BLACK, 0.6);
    let w = min_w.max(g.size().x + 40.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    let p = ui.painter();
    let (fill, border, bracket, ink) = if on {
        (Color32::from_rgb(0xd9, 0xd5, 0xc9), Color32::from_rgb(0xf0, 0xed, 0xe4), Color32::from_rgb(0x8d, 0x89, 0x81), ON_BAR)
    } else if resp.hovered() {
        (rgba(80, 75, 70, 0.85), EDGE, FRAME, INK)
    } else {
        (rgba(43, 40, 38, 0.85), EDGE, FRAME, INK)
    };
    p.rect_filled(rect, 0.0, fill);
    if on {
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(rect.left_top(), Color32::from_rgb(0xe2, 0xde, 0xd2));
        mesh.colored_vertex(rect.right_top(), Color32::from_rgb(0xe2, 0xde, 0xd2));
        mesh.colored_vertex(rect.right_bottom(), Color32::from_rgb(0xcf, 0xcb, 0xbf));
        mesh.colored_vertex(rect.left_bottom(), Color32::from_rgb(0xcf, 0xcb, 0xbf));
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        p.add(mesh);
    }
    p.rect_stroke(rect, 0.0, Stroke::new(2.0, border), StrokeKind::Inside);
    if !on {
        p.rect_stroke(rect.shrink(inset), 0.0, Stroke::new(1.0, INNER), StrokeKind::Inside);
    }
    let bw = inset + 2.0;
    for left in [true, false] {
        let x = if left { rect.left() + inset } else { rect.right() - inset };
        let x2 = if left { x + bw } else { x - bw };
        let (t, b) = (rect.top() + inset, rect.bottom() - inset);
        let s = Stroke::new(2.0, bracket);
        p.line(vec![pos2(x2, t), pos2(x, t), pos2(x, b), pos2(x2, b)], s);
    }
    let g = spaced(ui, text, size, ink, 0.6);
    p.galley(rect.center() - g.size() / 2.0 + vec2(0.0, 1.0), g, ink);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn square(ui: &mut Ui, text: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(62.0, 62.0), Sense::click());
    let r = rect.shrink(4.0);
    let p = ui.painter();
    p.rect_filled(r, 0.0, if resp.hovered() { rgba(233, 228, 216, 0.15) } else { rgba(30, 27, 25, 0.35) });
    p.rect_stroke(r, 0.0, Stroke::new(2.0, LINE), StrokeKind::Inside);
    p.rect_stroke(r.expand(4.0), 0.0, Stroke::new(1.0, LINE), StrokeKind::Inside);
    match text {
        "✕" => cross(p, r.center(), 9.0, INK),
        "←" => {
            let c = r.center();
            let st = Stroke::new(2.0, INK);
            p.line_segment([pos2(c.x - 11.0, c.y), pos2(c.x + 11.0, c.y)], st);
            p.line(vec![pos2(c.x - 4.0, c.y - 7.0), pos2(c.x - 11.0, c.y), pos2(c.x - 4.0, c.y + 7.0)], st);
        }
        _ => {
            p.text(r.center(), Align2::CENTER_CENTER, text, font(26.0), INK);
        }
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn cross(p: &egui::Painter, c: Pos2, half: f32, color: Color32) {
    let st = Stroke::new(2.0, color);
    p.line_segment([pos2(c.x - half, c.y - half), pos2(c.x + half, c.y + half)], st);
    p.line_segment([pos2(c.x - half, c.y + half), pos2(c.x + half, c.y - half)], st);
}

pub fn hexicon(ui: &mut Ui, rect: Rect) -> Response {
    let resp = ui.interact(rect, ui.scope_id().with("hexicon"), Sense::click());
    let hexagon = |r: Rect| {
        let (w, h) = (r.width(), r.height());
        vec![pos2(r.left() + 0.22 * w, r.top()), pos2(r.left() + 0.78 * w, r.top()), pos2(r.right(), r.top() + h / 2.0),
             pos2(r.left() + 0.78 * w, r.bottom()), pos2(r.left() + 0.22 * w, r.bottom()), pos2(r.left(), r.top() + h / 2.0)]
    };
    let p = ui.painter();
    p.add(Shape::convex_polygon(hexagon(rect), Color32::from_rgb(0x6f, 0x6a, 0x63), Stroke::NONE));
    let inner = if resp.hovered() { Color32::from_rgb(0xf2, 0xef, 0xe6) } else { Color32::from_rgb(0xd9, 0xd5, 0xca) };
    p.add(Shape::convex_polygon(hexagon(rect.shrink(3.0)), inner, Stroke::NONE));
    let c = rect.center();
    for dy in [-6.0, 0.0, 6.0] {
        p.line_segment([pos2(c.x - 9.0, c.y + dy), pos2(c.x + 9.0, c.y + dy)], Stroke::new(2.0, Color32::from_rgb(0x4b, 0x46, 0x41)));
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn page_background(p: &egui::Painter, r: Rect) {
    p.rect_filled(r, 0.0, PAGE);
    let blob = |cx: f32, cy: f32, rx: f32, ry: f32, a: f32| {
        let c = pos2(r.left() + cx * r.width(), r.top() + cy * r.height());
        let (rx, ry) = (rx * r.width() * 0.7, ry * r.height() * 0.7);
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(c, rgba(20, 17, 15, a));
        let n = 40;
        for i in 0..=n {
            let t = i as f32 / n as f32 * std::f32::consts::TAU;
            mesh.colored_vertex(pos2(c.x + rx * t.cos(), c.y + ry * t.sin()), Color32::TRANSPARENT);
        }
        for i in 1..=n as u32 {
            mesh.add_triangle(0, i, i + 1);
        }
        p.add(mesh);
    };
    blob(0.18, 0.12, 0.34, 0.22, 0.55);
    blob(0.52, 0.08, 0.30, 0.18, 0.5);
    blob(0.86, 0.14, 0.30, 0.20, 0.5);
    blob(0.30, 0.96, 0.40, 0.26, 0.45);
    blob(0.80, 0.98, 0.40, 0.26, 0.45);
    let mut y = r.top();
    while y < r.bottom() {
        p.line_segment([pos2(r.left(), y), pos2(r.right(), y)], Stroke::new(1.0, rgba(255, 255, 255, 0.025)));
        y += 4.0;
    }
}

pub fn field(ui: &mut Ui, label: &str, value: &mut String, hint: &str) -> Response {
    let mut changed = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 14.0;
        let (r, _) = ui.allocate_exact_size(vec2(130.0, 30.0), Sense::hover());
        ui.painter().text(pos2(r.left(), r.center().y), Align2::LEFT_CENTER, tr(label), font(15.0), SOFT);
        changed = Some(text_line(ui, value, hint, ui.available_width(), true));
    });
    changed.unwrap()
}

pub fn text_line(ui: &mut Ui, value: &mut String, hint: &str, width: f32, mono: bool) -> Response {
    let font_id = if mono { FontId::monospace(13.0) } else { font(15.0) };
    let resp = ui.add(egui::TextEdit::singleline(value).frame(egui::Frame::NONE).desired_width(width).font(font_id)
        .text_color(INK).hint_text(egui::RichText::new(tr(hint)).color(DIM)).margin(vec2(4.0, 6.0)));
    let color = if resp.has_focus() { INK } else { FRAME };
    ui.painter().line_segment([resp.rect.left_bottom(), resp.rect.right_bottom()], Stroke::new(1.0, color));
    resp
}

pub fn slider(ui: &mut Ui, value: &mut f64, min: f64, max: f64, step: f64, label: &str) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let w = ui.available_width() - 80.0;
        let (rect, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click_and_drag());
        if let Some(pos) = resp.interact_pointer_pos() {
            let t = ((pos.x - rect.left() - 10.0) / (rect.width() - 20.0)).clamp(0.0, 1.0) as f64;
            let v = ((min + t * (max - min)) / step).round() * step;
            if (v - *value).abs() > f64::EPSILON {
                *value = v;
                changed = true;
            }
        }
        let p = ui.painter();
        let track = Rect::from_min_max(pos2(rect.left() + 10.0, rect.center().y - 3.0), pos2(rect.right() - 10.0, rect.center().y + 3.0));
        p.rect_filled(track, 0.0, Color32::from_rgb(0x59, 0x55, 0x4f));
        let t = ((*value - min) / (max - min)).clamp(0.0, 1.0) as f32;
        let x = track.left() + t * track.width();
        p.rect_filled(Rect::from_min_max(track.min, pos2(x, track.max.y)), 0.0, LINE);
        let c = pos2(x, rect.center().y);
        p.add(Shape::convex_polygon(diamond_shape(c, 14.0), CREAM, Stroke::new(2.0, Color32::from_rgb(0x7d, 0x79, 0x72))));
        p.add(Shape::convex_polygon(diamond_shape(c, 9.0), Color32::TRANSPARENT, Stroke::new(1.5, Color32::from_rgb(0xb9, 0xb5, 0xab))));
        ui.add_space(18.0);
        ui.label(egui::RichText::new(label).font(font(16.0)).color(INK));
    });
    changed
}

pub fn tool(ui: &mut Ui, text: &str, enabled: bool, warn: bool) -> Response {
    let color = if warn { ALERT } else { INK };
    let border = if warn { Color32::from_rgb(0x9b, 0x6e, 0x6a) } else { Color32::from_rgb(0x7d, 0x79, 0x72) };
    let g = spaced(ui, tr(text), 14.0, color, 0.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 24.0, 28.0), if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    let fade = |c: Color32| if enabled { c } else { c.gamma_multiply(0.35) };
    if enabled && resp.hovered() {
        p.rect_filled(rect, 0.0, rgba(233, 228, 216, 0.12));
    }
    p.rect_stroke(rect, 0.0, Stroke::new(1.0, fade(border)), StrokeKind::Inside);
    p.galley(rect.center() - g.size() / 2.0, g, fade(color));
    resp
}

pub fn select(ui: &mut Ui, id: &str, value: &mut String, options: &[(String, String)]) -> bool {
    let shown = options.iter().find(|o| &o.0 == value).map(|o| tr(&o.1)).unwrap_or_default();
    let mut changed = false;
    egui::ComboBox::from_id_salt(id).selected_text(egui::RichText::new(shown).font(font(14.0)).color(INK)).show_ui(ui, |ui| {
        for (v, label) in options {
            if ui.selectable_label(v == value, egui::RichText::new(tr(label)).font(font(14.0))).clicked() {
                *value = v.clone();
                changed = true;
            }
        }
    });
    changed
}

pub fn progress(ui: &mut Ui, t: f32) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 8.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, Color32::from_rgb(0x59, 0x55, 0x4f));
    ui.painter().rect_filled(Rect::from_min_size(r.min, vec2(r.width() * t.clamp(0.0, 1.0), r.height())), 0.0, CREAM);
}

pub fn visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.panel_fill = PAGE;
    v.window_fill = DIALOG;
    v.extreme_bg_color = rgba(0, 0, 0, 0.18);
    v.override_text_color = Some(INK);
    v.selection.bg_fill = rgba(233, 228, 216, 0.25);
    v.selection.stroke = Stroke::new(1.0, INK);
    v.text_cursor.stroke = Stroke::new(2.0, INK);
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.bg_fill = DIALOG;
        w.weak_bg_fill = DIALOG;
        w.bg_stroke = Stroke::new(1.0, FRAME);
        w.fg_stroke = Stroke::new(1.0, INK);
        w.corner_radius = CornerRadius::ZERO;
    }
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, INK);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, FRAME);
    v.widgets.noninteractive.corner_radius = CornerRadius::ZERO;
    v.window_corner_radius = CornerRadius::ZERO;
    v.menu_corner_radius = CornerRadius::ZERO;
    v.window_stroke = Stroke::new(1.0, FRAME);
    v
}

