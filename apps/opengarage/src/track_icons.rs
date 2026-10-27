//! The pictograms of the track headers, one for each kind of instrument:
//! drawings of Game Icons (<https://game-icons.net>, CC BY 3.0; their
//! authors are named in "External files" in the documentation), kept as
//! SVG files in `apps/opengarage/track-icons`.
//!
//! Each drawing is one filled path. The program reads it, turns its curves
//! into short straight lines, fills it at the size it is shown and keeps
//! the picture, which is then painted in the colour asked for.

use std::collections::HashMap;
use std::f32::consts::PI;

use eframe::egui::{pos2, Color32, ColorImage, Id, Painter, Rect, TextureHandle, TextureOptions};

use crate::instruments::Instrument;

/// The drawings are made on a square of 512 units.
const DRAWN: f32 = 512.0;

fn svg(instrument: Instrument) -> &'static str {
    match instrument {
        Instrument::Piano => include_str!("../track-icons/grand-piano.svg"),
        Instrument::Synth => include_str!("../track-icons/piano-keys.svg"),
        Instrument::Bass => include_str!("../track-icons/guitar-bass-head.svg"),
        Instrument::Guitar => include_str!("../track-icons/guitar.svg"),
        Instrument::Strings => include_str!("../track-icons/violin.svg"),
        Instrument::Brass => include_str!("../track-icons/trumpet.svg"),
        Instrument::Winds => include_str!("../track-icons/saxophone.svg"),
        Instrument::Mallets => include_str!("../track-icons/xylophone.svg"),
        Instrument::Drums => include_str!("../track-icons/drum-kit.svg"),
        Instrument::Voice => include_str!("../track-icons/microphone.svg"),
        Instrument::Audio => include_str!("../track-icons/sound-waves.svg"),
        Instrument::Video => include_str!("../track-icons/film-strip.svg"),
        Instrument::Other => include_str!("../track-icons/musical-notes.svg"),
    }
}

/// The `d` attribute of the path of an SVG file.
fn path_data(svg: &str) -> &str {
    svg.split(" d=\"").nth(1).and_then(|rest| rest.split('"').next()).unwrap_or("")
}

/// Reads the numbers and letters of a path.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn skip(&mut self) {
        while self.bytes.get(self.at).is_some_and(|b| b.is_ascii_whitespace() || *b == b',') {
            self.at += 1;
        }
    }

    /// The next number: digits with a sign, a point and an exponent, each
    /// if any. "1.5.5" is two numbers, "-1-2" too.
    fn number(&mut self) -> Option<f32> {
        self.skip();
        let start = self.at;
        let at = |r: &Self| r.bytes.get(r.at).copied();
        if matches!(at(self), Some(b'+' | b'-')) {
            self.at += 1;
        }
        let mut digits = 0;
        while at(self).is_some_and(|b| b.is_ascii_digit()) {
            self.at += 1;
            digits += 1;
        }
        if at(self) == Some(b'.') {
            self.at += 1;
            while at(self).is_some_and(|b| b.is_ascii_digit()) {
                self.at += 1;
                digits += 1;
            }
        }
        if digits == 0 {
            self.at = start;
            return None;
        }
        if matches!(at(self), Some(b'e' | b'E')) && self.bytes.get(self.at + 1).is_some_and(|b| b.is_ascii_digit() || *b == b'-' || *b == b'+') {
            self.at += 2;
            while at(self).is_some_and(|b| b.is_ascii_digit()) {
                self.at += 1;
            }
        }
        std::str::from_utf8(&self.bytes[start..self.at]).ok()?.parse().ok()
    }

    /// The flag of an arc: one figure, which may be written against the next.
    fn flag(&mut self) -> Option<bool> {
        self.skip();
        let flag = match self.bytes.get(self.at) {
            Some(b'0') => false,
            Some(b'1') => true,
            _ => return None,
        };
        self.at += 1;
        Some(flag)
    }

    fn point(&mut self) -> Option<[f32; 2]> {
        Some([self.number()?, self.number()?])
    }
}

fn add(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] + b[0], a[1] + b[1]]
}

/// Into how many lines a curve through these points is cut.
fn pieces(points: &[[f32; 2]]) -> usize {
    let length: f32 = points.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum();
    ((length / 6.0).ceil() as usize).clamp(2, 32)
}

fn cubic(out: &mut Vec<[f32; 2]>, p0: [f32; 2], p1: [f32; 2], p2: [f32; 2], p3: [f32; 2]) {
    let n = pieces(&[p0, p1, p2, p3]);
    for i in 1..=n {
        let t = i as f32 / n as f32;
        let u = 1.0 - t;
        let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
        out.push([a * p0[0] + b * p1[0] + c * p2[0] + d * p3[0], a * p0[1] + b * p1[1] + c * p2[1] + d * p3[1]]);
    }
}

fn quadratic(out: &mut Vec<[f32; 2]>, p0: [f32; 2], p1: [f32; 2], p2: [f32; 2]) {
    let n = pieces(&[p0, p1, p2]);
    for i in 1..=n {
        let t = i as f32 / n as f32;
        let u = 1.0 - t;
        let (a, b, c) = (u * u, 2.0 * u * t, t * t);
        out.push([a * p0[0] + b * p1[0] + c * p2[0], a * p0[1] + b * p1[1] + c * p2[1]]);
    }
}

/// An arc of an ellipse as SVG writes it: from its two ends, its radii and
/// which of the four possible arcs it is (SVG 1.1, appendix F.6.5).
#[allow(clippy::too_many_arguments)]
fn arc(out: &mut Vec<[f32; 2]>, from: [f32; 2], radii: [f32; 2], turn: f32, large: bool, sweep: bool, to: [f32; 2]) {
    let (mut rx, mut ry) = (radii[0].abs(), radii[1].abs());
    if from == to {
        return;
    }
    if rx < 1e-6 || ry < 1e-6 {
        out.push(to);
        return;
    }
    let (sin, cos) = turn.to_radians().sin_cos();
    let (hx, hy) = ((from[0] - to[0]) / 2.0, (from[1] - to[1]) / 2.0);
    let (x1, y1) = (cos * hx + sin * hy, -sin * hx + cos * hy);
    // Radii too small to join the ends are grown just enough.
    let grow = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if grow > 1.0 {
        rx *= grow.sqrt();
        ry *= grow.sqrt();
    }
    let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let coef = (num / den).max(0.0).sqrt() * if large == sweep { -1.0 } else { 1.0 };
    let (cx1, cy1) = (coef * rx * y1 / ry, -coef * ry * x1 / rx);
    let centre = [cos * cx1 - sin * cy1 + (from[0] + to[0]) / 2.0, sin * cx1 + cos * cy1 + (from[1] + to[1]) / 2.0];
    let angle = |u: [f32; 2], v: [f32; 2]| (u[0] * v[1] - u[1] * v[0]).atan2(u[0] * v[0] + u[1] * v[1]);
    let start = [(x1 - cx1) / rx, (y1 - cy1) / ry];
    let first = angle([1.0, 0.0], start);
    let mut span = angle(start, [(-x1 - cx1) / rx, (-y1 - cy1) / ry]);
    if !sweep && span > 0.0 {
        span -= 2.0 * PI;
    } else if sweep && span < 0.0 {
        span += 2.0 * PI;
    }
    let n = ((span.abs() / (PI / 16.0)).ceil() as usize).max(2);
    for i in 1..n {
        let (s, c) = (first + span * i as f32 / n as f32).sin_cos();
        out.push([cos * rx * c - sin * ry * s + centre[0], sin * rx * c + cos * ry * s + centre[1]]);
    }
    out.push(to);
}

/// The outlines of a path, each a closed polygon, in the units of its drawing.
pub fn outlines(d: &str) -> Vec<Vec<[f32; 2]>> {
    let mut r = Reader { bytes: d.as_bytes(), at: 0 };
    let mut shapes = Vec::new();
    let mut shape: Vec<[f32; 2]> = Vec::new();
    let (mut here, mut start) = ([0.0f32; 2], [0.0f32; 2]);
    // The second control point of the last curve, for S and T, which mirror it.
    let mut last_cubic: Option<[f32; 2]> = None;
    let mut last_quadratic: Option<[f32; 2]> = None;
    let mut command = b'M';
    loop {
        r.skip();
        let Some(&next) = r.bytes.get(r.at) else { break };
        if next.is_ascii_alphabetic() {
            command = next;
            r.at += 1;
            if command.eq_ignore_ascii_case(&b'z') {
                if shape.len() > 2 {
                    shapes.push(std::mem::take(&mut shape));
                }
                shape.clear();
                here = start;
                last_cubic = None;
                last_quadratic = None;
                continue;
            }
        } else if command.eq_ignore_ascii_case(&b'z') {
            // Numbers after a Z: not a path that can be read further.
            break;
        }
        let relative = command.is_ascii_lowercase();
        let base = if relative { here } else { [0.0, 0.0] };
        let (cubic_was, quadratic_was) = (last_cubic.take(), last_quadratic.take());
        let read = match command.to_ascii_uppercase() {
            b'M' => r.point().map(|p| {
                if shape.len() > 2 {
                    shapes.push(std::mem::take(&mut shape));
                }
                shape.clear();
                here = add(base, p);
                start = here;
                shape.push(here);
                // The pairs after a move are lines.
                command = if relative { b'l' } else { b'L' };
            }),
            b'L' => r.point().map(|p| {
                here = add(base, p);
                shape.push(here);
            }),
            b'H' => r.number().map(|x| {
                here[0] = base[0] + x;
                shape.push(here);
            }),
            b'V' => r.number().map(|y| {
                here[1] = base[1] + y;
                shape.push(here);
            }),
            b'C' => (|| Some((r.point()?, r.point()?, r.point()?)))().map(|(a, b, p)| {
                let (a, b, p) = (add(base, a), add(base, b), add(base, p));
                cubic(&mut shape, here, a, b, p);
                last_cubic = Some(b);
                here = p;
            }),
            b'S' => (|| Some((r.point()?, r.point()?)))().map(|(b, p)| {
                let a = cubic_was.map_or(here, |c| [2.0 * here[0] - c[0], 2.0 * here[1] - c[1]]);
                let (b, p) = (add(base, b), add(base, p));
                cubic(&mut shape, here, a, b, p);
                last_cubic = Some(b);
                here = p;
            }),
            b'Q' => (|| Some((r.point()?, r.point()?)))().map(|(a, p)| {
                let (a, p) = (add(base, a), add(base, p));
                quadratic(&mut shape, here, a, p);
                last_quadratic = Some(a);
                here = p;
            }),
            b'T' => r.point().map(|p| {
                let a = quadratic_was.map_or(here, |c| [2.0 * here[0] - c[0], 2.0 * here[1] - c[1]]);
                let p = add(base, p);
                quadratic(&mut shape, here, a, p);
                last_quadratic = Some(a);
                here = p;
            }),
            b'A' => (|| Some((r.point()?, r.number()?, r.flag()?, r.flag()?, r.point()?)))().map(|(radii, turn, large, sweep, p)| {
                let p = add(base, p);
                arc(&mut shape, here, radii, turn, large, sweep, p);
                here = p;
            }),
            _ => None,
        };
        if read.is_none() {
            break;
        }
    }
    if shape.len() > 2 {
        shapes.push(shape);
    }
    shapes
}

/// How much of each pixel of a `width` by `height` picture the outlines
/// cover, from 0 to 255, filled the way SVG fills by default (a point is
/// inside when the outlines turn around it). The drawing is fitted to the
/// picture and centred in it.
pub fn coverage(shapes: &[Vec<[f32; 2]>], width: usize, height: usize) -> Vec<u8> {
    const ROWS: usize = 4;
    let scale = width.min(height) as f32 / DRAWN;
    let (dx, dy) = ((width as f32 - DRAWN * scale) / 2.0, (height as f32 - DRAWN * scale) / 2.0);
    let edges: Vec<([f32; 2], [f32; 2])> = shapes
        .iter()
        .flat_map(|shape| {
            let ends = shape.iter().skip(1).chain(shape.first());
            shape.iter().zip(ends).map(|(a, b)| ([a[0] * scale + dx, a[1] * scale + dy], [b[0] * scale + dx, b[1] * scale + dy]))
        })
        .filter(|(a, b)| a[1] != b[1])
        .collect();
    let mut out = vec![0u8; width * height];
    let mut row = vec![0.0f32; width];
    let mut crossings: Vec<(f32, i32)> = Vec::new();
    for y in 0..height {
        row.iter_mut().for_each(|c| *c = 0.0);
        for sub in 0..ROWS {
            let at = y as f32 + (sub as f32 + 0.5) / ROWS as f32;
            crossings.clear();
            for (a, b) in &edges {
                let (low, high) = if a[1] < b[1] { (a, b) } else { (b, a) };
                if at >= low[1] && at < high[1] {
                    let x = a[0] + (at - a[1]) * (b[0] - a[0]) / (b[1] - a[1]);
                    crossings.push((x, if b[1] > a[1] { 1 } else { -1 }));
                }
            }
            crossings.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut winding = 0;
            for pair in crossings.windows(2) {
                winding += pair[0].1;
                if winding == 0 {
                    continue;
                }
                // The span between two crossings, inside: each pixel gets
                // the part of it that it holds.
                let (from, to) = (pair[0].0.clamp(0.0, width as f32), pair[1].0.clamp(0.0, width as f32));
                let mut x = from.floor() as usize;
                while (x as f32) < to && x < width {
                    let part = (to.min(x as f32 + 1.0) - from.max(x as f32)).max(0.0);
                    row[x] += part / ROWS as f32;
                    x += 1;
                }
            }
        }
        for (x, c) in row.iter().enumerate() {
            out[y * width + x] = (c.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
    out
}

/// The pictures already made, by instrument and size in pixels.
#[derive(Clone, Default)]
struct Made(HashMap<(u8, usize, usize), TextureHandle>);

/// Paints the pictogram of an instrument over `rect`, in `ink`.
pub fn paint(painter: &Painter, rect: Rect, instrument: Instrument, ink: Color32) {
    let ctx = painter.ctx();
    let size = rect.size() * ctx.pixels_per_point();
    let (width, height) = (size.x.round().max(1.0) as usize, size.y.round().max(1.0) as usize);
    let key = (instrument as u8, width, height);
    let id = Id::new("track icons");
    let texture = ctx.data_mut(|d| d.get_temp_mut_or_default::<Made>(id).0.get(&key).cloned());
    let texture = texture.unwrap_or_else(|| {
        let alpha = coverage(&outlines(path_data(svg(instrument))), width, height);
        let image = ColorImage { size: [width, height], pixels: alpha.into_iter().map(Color32::from_white_alpha).collect() };
        let texture = ctx.load_texture(format!("track icon {key:?}"), image, TextureOptions::LINEAR);
        ctx.data_mut(|d| d.get_temp_mut_or_default::<Made>(id).0.insert(key, texture.clone()));
        texture
    });
    painter.image(texture.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), ink);
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Instrument; 12] = [
        Instrument::Piano,
        Instrument::Synth,
        Instrument::Bass,
        Instrument::Guitar,
        Instrument::Strings,
        Instrument::Brass,
        Instrument::Winds,
        Instrument::Mallets,
        Instrument::Drums,
        Instrument::Voice,
        Instrument::Audio,
        Instrument::Other,
    ];

    #[test]
    fn a_square_and_a_ring_are_filled_as_svg_fills_them() {
        // A square of half the drawing, in the middle: a quarter of the picture.
        let square = outlines("M128 128h256v256H128z");
        assert_eq!(square, vec![vec![[128.0, 128.0], [384.0, 128.0], [384.0, 384.0], [128.0, 384.0]]]);
        let cover = coverage(&square, 16, 16);
        let full = cover.iter().filter(|c| **c == 255).count();
        assert_eq!(full, 64, "{cover:?}");
        assert_eq!(cover[0], 0);
        // A circle drawn with two arcs, its hole drawn the other way round: a ring.
        let ring = outlines("M256 56a200 200 0 1 1 0 400a200 200 0 1 1 0-400zm0 100a100 100 0 1 0 0 200a100 100 0 1 0 0-200z");
        assert_eq!(ring.len(), 2);
        let cover = coverage(&ring, 64, 64);
        assert_eq!(cover[32 * 64 + 32], 0, "the hole");
        assert_eq!(cover[32 * 64 + 10], 255, "the ring");
        assert_eq!(cover[2 * 64 + 2], 0, "outside");
    }

    #[test]
    fn numbers_written_against_each_other_are_read_apart() {
        let shape = outlines("M10-20l.5.5-1e1 0z");
        assert_eq!(shape, vec![vec![[10.0, -20.0], [10.5, -19.5], [0.5, -19.5]]]);
    }

    #[test]
    fn every_pictogram_is_read_whole_and_fills_part_of_its_badge() {
        for instrument in ALL {
            let d = path_data(svg(instrument));
            assert!(d.len() > 100, "{instrument:?}");
            let shapes = outlines(d);
            assert!(!shapes.is_empty(), "{instrument:?}");
            // Every point within the drawing: nothing read askew.
            for p in shapes.iter().flatten() {
                assert!((-1.0..=513.0).contains(&p[0]) && (-1.0..=513.0).contains(&p[1]), "{instrument:?}: {p:?}");
            }
            let cover = coverage(&shapes, 40, 40);
            let inked = cover.iter().map(|c| f32::from(*c) / 255.0).sum::<f32>() / 1600.0;
            assert!(inked > 0.1 && inked < 0.75, "{instrument:?}: {inked}");
        }
    }
}
