//! The track area: ruler, global tracks, track headers, region lanes, automation.

use crate::i18n::tr;
use crate::widgets::{channel_items, device_items, radio_list};
use crate::track_icons;
use crate::userloops::LoopPick;
use eframe::egui::{
    self, pos2, vec2, Align, Align2, Color32, CursorIcon, FontId, Layout, Painter, Pos2, Rect,
    Response, Sense, Shape,
};
use og_engine::{Content, DrummerParams, Marker, Note, Point, Region, TrackKind};

use crate::app::{
    accepts, snap, stroke, track_color, App, AutoParam, CurveId, Dialog, Drag, TrackAction, EDGE,
    palette, GLOBAL_H, HEADER_W, LANE_H, RULER_H, SNAP, TRACK_COLORS,
};

/// How a curve lane maps values to its height.
pub struct LaneSpec {
    pub min: f64,
    pub max: f64,
    pub default: f64,
    /// Values hold until the next point instead of ramping.
    pub step: bool,
    /// Values snap to multiples of this; 0 leaves them free.
    pub round: f64,
    pub color: Color32,
    pub unit: &'static str,
}

fn kind_badge(kind: TrackKind) -> &'static str {
    match kind {
        TrackKind::Instrument => tr("Instr."),
        TrackKind::Audio => tr("Audio"),
        TrackKind::Guitar => tr("Guitar"),
        TrackKind::Drummer => tr("Drummer"),
        TrackKind::Video => tr("Video"),
    }
}

impl App {
    fn x_of(&self, rect: Rect, beat: f64) -> f32 {
        rect.left() + ((beat - self.scroll) * f64::from(self.ppb)) as f32
    }

    fn beat_of(&self, rect: Rect, x: f32) -> f64 {
        self.scroll + f64::from(x - rect.left()) / f64::from(self.ppb)
    }

    /// Grid used when moving regions: finer once zoomed in.
    fn region_grid(&self) -> f64 {
        if self.ppb >= 48.0 {
            SNAP
        } else {
            1.0
        }
    }

    fn draw_grid(&self, ui: &egui::Ui, painter: &Painter, rect: Rect) {
        let line = ui.visuals().widgets.noninteractive.bg_stroke.color;
        let bar_len = self.project.bar_len();
        let ppb = f64::from(self.ppb);
        painter.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
        let first = self.scroll.floor().max(0.0) as i64;
        let last = (self.scroll + f64::from(rect.width()) / ppb).ceil() as i64;
        for beat in first..=last {
            let in_bar = (beat as f64) % bar_len;
            let on_bar = in_bar < 1e-6;
            if !on_bar && ppb < 10.0 {
                continue;
            }
            let x = self.x_of(rect, beat as f64);
            let color = if on_bar { line } else { line.gamma_multiply(0.35) };
            painter.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], stroke(1.0, color));
        }
        // Past the end of the project.
        let end = self.x_of(rect, self.project.end_beat());
        if end < rect.right() {
            let after = Rect::from_min_max(pos2(end.max(rect.left()), rect.top()), rect.max);
            painter.rect_filled(after, 0.0, Color32::from_black_alpha(if ui.visuals().dark_mode { 70 } else { 26 }));
        }
    }

    fn draw_playhead(&self, ui: &egui::Ui, painter: &Painter, rect: Rect) {
        let x = self.x_of(rect, self.position);
        let color = if self.recording { crate::theme::current().record() } else { ui.visuals().text_color() };
        painter.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], stroke(1.5, color));
    }

    /// A fixed-size cell in the header column.
    fn header_cell(ui: &mut egui::Ui, height: f32, add: impl FnOnce(&mut egui::Ui)) -> Response {
        ui.allocate_ui_with_layout(vec2(HEADER_W, height), Layout::left_to_right(Align::Center), |ui| {
            ui.set_min_size(vec2(HEADER_W, height));
            ui.set_max_width(HEADER_W);
            add(ui);
        })
        .response
    }

    /// A loop being dragged from the loop browser over the tracks: a line
    /// shows the bar it would start on; released on a track it goes there,
    /// released under the tracks it gets a track of its own.
    fn loop_drop(&mut self, ui: &egui::Ui) {
        let Some(pick) = egui::DragAndDrop::payload::<LoopPick>(ui.ctx()) else { return };
        let Some(pointer) = ui.ctx().pointer_interact_pos().filter(|p| self.tracks_rect.contains(*p) && p.x >= self.lane_left) else { return };
        let bar = self.project.bar_len();
        let beat = self.scroll + f64::from((pointer.x - self.lane_left) / self.ppb);
        let start = ((beat / bar).floor() * bar).max(0.0);
        let x = self.lane_left + ((start - self.scroll) * f64::from(self.ppb)) as f32;
        let target = self.lane_rects.iter().position(|lane| lane.y_range().contains(pointer.y));
        let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("loop_drop")));
        let span = target.map_or(self.tracks_rect.y_range(), |ti| self.lane_rects[ti].y_range());
        painter.vline(x, span, stroke(2.0, palette(2)));
        if ui.input(|i| i.pointer.any_released()) {
            self.add_loop_at(*pick, target, start);
        }
    }

    pub fn tracks_area(&mut self, ui: &mut egui::Ui) {
        self.tracks_rect = ui.max_rect();
        if ui.rect_contains_pointer(ui.max_rect()) {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta()));
            if zoom != 1.0 {
                self.ppb = (self.ppb * zoom).clamp(6.0, 160.0);
            }
            if scroll.x != 0.0 {
                self.scroll = (self.scroll - f64::from(scroll.x / self.ppb)).max(0.0);
            }
        }
        if self.playing {
            let visible = f64::from(self.lane_w / self.ppb);
            if !self.settings.tracks_playhead_moves {
                // The playhead stays in the middle and the tracks slide under
                // it, once the song has played as far as the middle.
                self.scroll = (self.position - visible / 2.0).max(0.0);
            } else if self.position < self.scroll || self.position > self.scroll + visible {
                // Page the view when the playhead leaves it.
                self.scroll = self.position;
            }
        }

        ui.horizontal(|ui| {
            Self::header_cell(ui, RULER_H, |ui| {
                if ui.button(tr("+ Track")).clicked() {
                    self.dialog = Some(Dialog::NewTrack);
                }
                ui.spacing_mut().slider_width = 70.0;
                ui.add(egui::Slider::new(&mut self.ppb, 6.0..=160.0).show_value(false)).on_hover_text(tr("Zoom"));
                if let Some(param) = &mut self.automation {
                    egui::ComboBox::from_id_salt("auto_param").width(110.0).selected_text(param.label()).show_ui(ui, |ui| {
                        for candidate in AutoParam::all() {
                            ui.selectable_value(param, candidate, candidate.label());
                        }
                    });
                }
            });
            self.ruler(ui);
        });

        if self.show_global {
            ui.horizontal(|ui| {
                Self::header_cell(ui, GLOBAL_H, |ui| {
                    ui.label(tr("Arrangement"));
                    ui.weak(tr("double-click: section; moving it moves its contents"));
                });
                self.arrangement_lane(ui);
            });
            let tempo = self.project.tempo;
            let rows = [
                (tr("Tempo"), CurveId::Tempo, LaneSpec { min: 40.0, max: 240.0, default: tempo, step: true, round: 1.0, color: palette(1), unit: "bpm" }),
                (tr("Transposition"), CurveId::Transpose, LaneSpec { min: -12.0, max: 12.0, default: 0.0, step: true, round: 1.0, color: palette(3), unit: tr("semitones") }),
            ];
            for (label, id, spec) in rows {
                ui.horizontal(|ui| {
                    Self::header_cell(ui, GLOBAL_H, |ui| {
                        ui.label(label);
                        ui.weak(tr("click: point"));
                    });
                    self.curve_row(ui, GLOBAL_H, id, &spec);
                });
            }
        }

        self.lane_rects.clear();
        egui::ScrollArea::vertical().id_salt("tracks").auto_shrink([false, false]).show(ui, |ui| {
            for ti in 0..self.project.tracks.len() {
                ui.horizontal(|ui| {
                    self.track_header(ui, ti);
                    self.lane(ui, ti);
                });
            }
            // Structural changes asked from a header menu wait until every row is
            // drawn: the rows above were laid out for the old list of tracks.
            match self.pending_track.take() {
                Some((ti, TrackAction::Delete)) => self.delete_track(ti),
                Some((ti, TrackAction::Duplicate)) => self.duplicate_track(ti),
                Some((ti, TrackAction::Move(by))) => self.move_track(ti, by),
                None => {}
            }
            if self.show_master {
                ui.horizontal(|ui| {
                    Self::header_cell(ui, GLOBAL_H, |ui| {
                        ui.label(tr("Master track"));
                        ui.spacing_mut().slider_width = 90.0;
                        ui.add(egui::Slider::new(&mut self.project.master_volume, 0.0..=1.2).show_value(false));
                    });
                    let volume = f64::from(self.project.master_volume);
                    let spec = LaneSpec { min: 0.0, max: 1.2, default: volume, step: false, round: 0.0, color: palette(4), unit: "" };
                    self.curve_row(ui, GLOBAL_H, CurveId::Master, &spec);
                });
            }
            if self.project.tracks.is_empty() {
                ui.add_space(20.0);
                ui.label(tr("No track. Click “+ Track” to begin."));
            }
        });
        self.apply_region_drag(ui.ctx());
        self.loop_drop(ui);
    }

    fn ruler(&mut self, ui: &mut egui::Ui) {
        let (resp, painter) = ui.allocate_painter(vec2(ui.available_width(), RULER_H), Sense::click_and_drag());
        let rect = resp.rect;
        self.lane_left = rect.left();
        let painter = painter.with_clip_rect(rect);
        let text_color = ui.visuals().text_color();
        let line = ui.visuals().widgets.noninteractive.bg_stroke.color;
        painter.rect_filled(rect, 0.0, ui.visuals().faint_bg_color);

        let strip = rect.top() + 9.0;
        if self.project.cycle_end > self.project.cycle_start {
            let area = Rect::from_min_max(
                pos2(self.x_of(rect, self.project.cycle_start), rect.top()),
                pos2(self.x_of(rect, self.project.cycle_end), strip),
            );
            let color = if self.project.cycle_enabled { Color32::from_rgb(235, 190, 60) } else { line };
            painter.rect_filled(area, 2.0, color);
        }

        let bar_len = self.project.bar_len();
        let bar_px = (bar_len * f64::from(self.ppb)) as f32;
        // Label every bar, or every 2nd, 4th… when zoomed out.
        let mut every = 1usize;
        while bar_px * (every as f32) < 34.0 {
            every *= 2;
        }
        let first = (self.scroll / bar_len).floor().max(0.0) as usize;
        let last = ((self.scroll + f64::from(rect.width() / self.ppb)) / bar_len).ceil() as usize;
        for bar in first..=last {
            let x = self.x_of(rect, bar as f64 * bar_len);
            let labelled = bar % every == 0;
            let top = if labelled { strip + 1.0 } else { rect.bottom() - 6.0 };
            painter.line_segment([pos2(x, top), pos2(x, rect.bottom())], stroke(1.0, line));
            if labelled {
                painter.text(pos2(x + 4.0, rect.bottom() - 2.0), Align2::LEFT_BOTTOM, (bar + 1).to_string(), FontId::proportional(11.0), text_color);
            }
        }
        let end = self.x_of(rect, self.project.end_beat());
        painter.line_segment([pos2(end, strip), pos2(end, rect.bottom())], stroke(2.0, palette(5)));
        let x = self.x_of(rect, self.position);
        painter.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], stroke(2.0, text_color));

        // Upper strip: drag to set the cycle. Rest of the ruler: move the playhead.
        if resp.drag_started() {
            if let Some(p) = ui.input(|i| i.pointer.press_origin()) {
                if p.y <= strip {
                    self.drag = Some(Drag::Cycle { anchor: snap(self.beat_of(rect, p.x), 1.0).max(0.0) });
                }
            }
        }
        let cycle_drag = match &self.drag {
            Some(Drag::Cycle { anchor }) => Some(*anchor),
            _ => None,
        };
        if let Some(p) = resp.interact_pointer_pos() {
            let beat = self.beat_of(rect, p.x);
            if let Some(anchor) = cycle_drag {
                if resp.dragged() {
                    let other = snap(beat, 1.0).max(0.0);
                    if (other - anchor).abs() >= 1.0 {
                        self.project.cycle_start = anchor.min(other);
                        self.project.cycle_end = anchor.max(other);
                        self.project.cycle_enabled = true;
                    }
                }
            } else if resp.clicked() || resp.dragged() {
                if resp.clicked() && p.y <= strip {
                    self.project.cycle_enabled = !self.project.cycle_enabled;
                } else {
                    self.go_to(snap(beat, SNAP));
                }
            }
        }
        if resp.drag_stopped() && cycle_drag.is_some() {
            self.drag = None;
        }
    }

    fn arrangement_lane(&mut self, ui: &mut egui::Ui) {
        let (resp, painter) = ui.allocate_painter(vec2(ui.available_width(), GLOBAL_H), Sense::click_and_drag());
        let rect = resp.rect;
        let painter = painter.with_clip_rect(rect);
        self.draw_grid(ui, &painter, rect);
        let bar = self.project.bar_len();
        for (i, marker) in self.project.markers.iter().enumerate() {
            let body = Rect::from_min_max(
                pos2(self.x_of(rect, marker.start), rect.top() + 3.0),
                pos2(self.x_of(rect, marker.start + marker.length), rect.bottom() - 3.0),
            );
            painter.rect_filled(body, 4.0, palette(i + 2).gamma_multiply(0.8));
            painter.with_clip_rect(body.intersect(rect)).text(
                body.left_center() + vec2(6.0, 0.0),
                Align2::LEFT_CENTER,
                &marker.name,
                FontId::proportional(12.0),
                Color32::from_gray(20),
            );
        }
        self.draw_playhead(ui, &painter, rect);

        let hit = |app: &App, beat: f64| app.project.markers.iter().rposition(|m| beat >= m.start && beat < m.start + m.length);
        if resp.drag_started() {
            if let Some(p) = ui.input(|i| i.pointer.press_origin()) {
                let beat = self.beat_of(rect, p.x);
                if let Some(index) = hit(self, beat) {
                    let marker = &self.project.markers[index];
                    let resize = self.x_of(rect, marker.start + marker.length) - p.x <= EDGE;
                    let grab = beat - marker.start;
                    self.checkpoint();
                    self.drag = Some(Drag::Marker { index, grab, resize });
                }
            }
        }
        if let (true, Some(Drag::Marker { index, grab, resize }), Some(p)) = (resp.dragged(), self.drag.clone(), resp.interact_pointer_pos()) {
            let beat = self.beat_of(rect, p.x);
            if !resize {
                // The regions of the section travel with it.
                self.move_section(index, snap(beat - grab, bar).max(0.0));
            } else if let Some(marker) = self.project.markers.get_mut(index) {
                marker.length = (snap(beat, bar) - marker.start).max(bar);
            }
        }
        if resp.drag_stopped() && matches!(self.drag, Some(Drag::Marker { .. })) {
            self.drag = None;
        }
        if let Some(p) = resp.interact_pointer_pos() {
            let beat = self.beat_of(rect, p.x);
            let target = hit(self, beat);
            if resp.double_clicked() && target.is_none() {
                self.checkpoint();
                let start = (beat / bar).floor().max(0.0) * bar;
                let name = tf!("Section {}", self.project.markers.len() + 1);
                self.project.markers.push(Marker { name, start, length: bar * 8.0 });
            }
            if resp.secondary_clicked() {
                self.ctx_index = target;
            }
        }
        resp.context_menu(|ui| {
            let Some(marker) = self.ctx_index.and_then(|i| self.project.markers.get_mut(i)) else {
                ui.weak(tr("Double-click to add a section."));
                return;
            };
            ui.label(tr("Section name"));
            ui.text_edit_singleline(&mut marker.name);
            for name in [tr("Intro"), tr("Verse"), tr("Refrain"), tr("Bridge"), tr("Outro")] {
                if ui.button(name).clicked() {
                    marker.name = name.to_owned();
                    ui.close_menu();
                }
            }
            ui.separator();
            if ui.button(tr("Repeat the section right after")).clicked() {
                if let Some(i) = self.ctx_index {
                    self.duplicate_section(i);
                }
                ui.close_menu();
            }
            if ui.button(tr("Delete the marker only")).clicked() {
                if let Some(i) = self.ctx_index.take() {
                    self.delete_section(i, false);
                }
                ui.close_menu();
            }
            if ui.button(tr("Delete the section and its contents")).clicked() {
                if let Some(i) = self.ctx_index.take() {
                    self.delete_section(i, true);
                }
                ui.close_menu();
            }
        });
    }

    /// A lane showing one curve on the grid.
    fn curve_row(&mut self, ui: &mut egui::Ui, height: f32, id: CurveId, spec: &LaneSpec) {
        let (resp, painter) = ui.allocate_painter(vec2(ui.available_width(), height), Sense::click_and_drag());
        let rect = resp.rect;
        let painter = painter.with_clip_rect(rect);
        self.draw_grid(ui, &painter, rect);
        self.point_lane(ui, &resp, &painter, rect, id, spec);
        self.draw_playhead(ui, &painter, rect);
    }

    /// Draws a curve over `rect` and lets the pointer add, move and remove its points.
    pub fn point_lane(&mut self, ui: &egui::Ui, resp: &Response, painter: &Painter, rect: Rect, id: CurveId, spec: &LaneSpec) {
        let points: Vec<Point> = self.curve(id).to_vec();
        let (top, bottom) = (rect.top() + 5.0, rect.bottom() - 5.0);
        let span = (spec.max - spec.min).max(1e-9);
        let y_of = |value: f64| bottom - ((value - spec.min) / span).clamp(0.0, 1.0) as f32 * (bottom - top);
        let value_of = |y: f32| {
            let raw = spec.min + f64::from((bottom - y) / (bottom - top)).clamp(0.0, 1.0) * span;
            if spec.round > 0.0 { (raw / spec.round).round() * spec.round } else { raw }
        };
        // Copies, so the closures below do not hold on to `self`.
        let (left, scroll, ppb) = (rect.left(), self.scroll, f64::from(self.ppb));
        let place = |p: &Point| pos2(left + ((p.beat - scroll) * ppb) as f32, y_of(p.value));

        let mut path = Vec::with_capacity(points.len() * 2 + 2);
        match points.first() {
            None => path.push(pos2(rect.left(), y_of(spec.default))),
            Some(first) if spec.step => path.push(pos2(rect.left(), y_of(spec.default)).min(pos2(place(first).x, f32::MAX))),
            Some(first) => path.push(pos2(rect.left().min(place(first).x), y_of(first.value))),
        }
        for point in &points {
            let at = place(point);
            if spec.step {
                if let Some(prev) = path.last().copied() {
                    path.push(pos2(at.x, prev.y));
                }
            }
            path.push(at);
        }
        if let Some(last) = path.last().copied() {
            path.push(pos2(rect.right().max(last.x), last.y));
        }
        let color = if points.is_empty() { spec.color.gamma_multiply(0.45) } else { spec.color };
        painter.add(Shape::line(path, stroke(1.5, color)));
        for point in &points {
            painter.circle_filled(place(point), 3.5, spec.color);
            painter.circle_stroke(place(point), 3.5, stroke(1.0, Color32::from_black_alpha(160)));
        }

        let hit = |p: Pos2| points.iter().position(|pt| (place(pt) - p).length() <= 8.0);
        if resp.drag_started() {
            if let Some(index) = ui.input(|i| i.pointer.press_origin()).and_then(hit) {
                self.checkpoint();
                self.drag = Some(Drag::Point { curve: id, index });
            }
        }
        let dragging = match &self.drag {
            Some(Drag::Point { curve, index }) if *curve == id => Some(*index),
            _ => None,
        };
        if let (true, Some(index), Some(p)) = (resp.dragged(), dragging, resp.interact_pointer_pos()) {
            // A point stays between its neighbours.
            let low = if index > 0 { points[index - 1].beat } else { 0.0 };
            let high = points.get(index + 1).map_or(f64::MAX, |n| n.beat);
            let beat = snap(self.beat_of(rect, p.x), SNAP).clamp(low, high);
            let value = value_of(p.y);
            if let Some(point) = self.curve_mut(id).and_then(|c| c.get_mut(index)) {
                *point = Point { beat, value };
            }
        }
        if resp.drag_stopped() && dragging.is_some() {
            self.drag = None;
        }
        if let Some(p) = resp.interact_pointer_pos() {
            let target = hit(p);
            if resp.clicked() && target.is_none() {
                let point = Point { beat: snap(self.beat_of(rect, p.x), SNAP).max(0.0), value: value_of(p.y) };
                self.checkpoint();
                if let Some(curve) = self.curve_mut(id) {
                    let at = curve.partition_point(|c| c.beat <= point.beat);
                    curve.insert(at, point);
                }
            } else if let (true, Some(index)) = (resp.secondary_clicked(), target) {
                self.checkpoint();
                if let Some(curve) = self.curve_mut(id) {
                    curve.remove(index);
                }
            }
        }
        if let Some(p) = resp.hover_pos() {
            let value = hit(p).map_or_else(|| value_of(p.y), |i| points[i].value);
            let text = if spec.round >= 1.0 { format!("{value:.0} {}", spec.unit) } else { format!("{value:.2} {}", spec.unit) };
            resp.clone().on_hover_text_at_pointer(text);
        }
    }

    fn track_header(&mut self, ui: &mut egui::Ui, ti: usize) {
        if ti >= self.project.tracks.len() {
            return;
        }
        let selected = self.selected_track == ti || (self.editor_tracks.len() > 1 && self.editor_tracks.contains(&ti));
        let peak = self.track_peaks.get(ti).copied().unwrap_or(0.0);
        let mut open_library = false;
        let mut make_lead = false;
        let inner = ui.allocate_ui_with_layout(vec2(HEADER_W, LANE_H), Layout::top_down(Align::Min), |ui| {
            ui.set_min_size(vec2(HEADER_W, LANE_H));
            ui.set_max_width(HEADER_W);
            // A click on a word of the header selects the track, not the word.
            ui.style_mut().interaction.selectable_labels = false;
            let cell = ui.max_rect();
            let fill = if selected { ui.visuals().selection.bg_fill.gamma_multiply(0.4) } else { ui.visuals().faint_bg_color };
            ui.painter().rect_filled(cell, 4.0, fill);
            // Level meter along the right edge.
            let meter = Rect::from_min_max(pos2(cell.right() - 6.0, cell.top() + 4.0), pos2(cell.right() - 2.0, cell.bottom() - 4.0));
            ui.painter().rect_filled(meter, 1.0, ui.visuals().extreme_bg_color);
            let mut level = meter;
            level.set_top(meter.bottom() - meter.height() * peak.min(1.0));
            ui.painter().rect_filled(level, 1.0, if peak > 0.95 { palette(5) } else { palette(0) });

            let instrument = self.track_instrument(ti);
            // With the original sound, M says whether the track plays along.
            let recording = self.is_recording_track(ti);
            let along = self.project.follow && !recording;
            let recordings = (0..self.project.tracks.len()).filter(|k| self.is_recording_track(*k)).count();
            let leading = og_engine::recording::lead(&self.project) == Some(ti);
            let track = &mut self.project.tracks[ti];
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                // The pictogram says what the track plays; a click chooses its colour.
                let (badge, badge_resp) = ui.allocate_exact_size(vec2(22.0, 20.0), Sense::click());
                ui.painter().rect_filled(badge, 4.0, track_color(track.color));
                track_icons::paint(ui.painter(), badge.shrink2(vec2(3.0, 2.0)), instrument, Color32::from_gray(25));
                let label = instrument.label();
                let badge_resp = badge_resp.on_hover_text(tf!("{label} · click: choose the colour of the track" [label]));
                let colours = ui.make_persistent_id(("track colours", ti));
                if badge_resp.clicked() {
                    ui.memory_mut(|m| m.toggle_popup(colours));
                }
                egui::popup_below_widget(ui, colours, &badge_resp, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                    ui.horizontal(|ui| {
                        for colour in 0..TRACK_COLORS as u8 {
                            let (swatch, chosen) = ui.allocate_exact_size(vec2(20.0, 18.0), Sense::click());
                            ui.painter().rect_filled(swatch, 4.0, track_color(colour));
                            if colour == track.color {
                                ui.painter().rect_stroke(swatch.expand(1.5), 5.0, stroke(2.0, ui.visuals().strong_text_color()));
                            }
                            if chosen.clicked() {
                                track.color = colour;
                            }
                        }
                    });
                });
                ui.add(egui::TextEdit::singleline(&mut track.name).desired_width(92.0));
                if along {
                    let mut silent = !track.with_recording;
                    if ui.toggle_value(&mut silent, "M").on_hover_text(tr("Silent while the song plays with its original sound: turn it off to play this track along with the recording")).changed() {
                        track.with_recording = !silent;
                    }
                } else {
                    ui.toggle_value(&mut track.mute, "M").on_hover_text(tr("Mute"));
                }
                ui.toggle_value(&mut track.solo, "S").on_hover_text(tr("Solo"));
                ui.toggle_value(&mut track.locked, tr("L")).on_hover_text(tr("Lock track"));
                if track.kind.is_audio() {
                    ui.toggle_value(&mut track.monitor, tr("I")).on_hover_text(tr("Listen to the input through the track's effects (on headphones, to avoid feedback)"));
                } else {
                    ui.weak(kind_badge(track.kind));
                }
            });
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                if recording {
                    // A recording of the song: which one leads, when there are several.
                    let what = if track.kind == TrackKind::Video { "YouTube" } else { tr("Recording") };
                    if recordings > 1 {
                        let mut lead = leading;
                        if ui.add_sized([100.0, 18.0], egui::SelectableLabel::new(lead, tf!("{what} · leads" [what]))).on_hover_text(tr("The recording the song follows with its original sound")).clicked() && !lead {
                            lead = true;
                            make_lead = lead;
                        }
                    } else {
                        ui.add_sized([100.0, 18.0], egui::Label::new(what));
                    }
                } else {
                let patch = if track.patch.is_empty() { tr("Sound…") } else { tr(track.patch.as_str()) };
                if ui.add_sized([100.0, 18.0], egui::Button::new(patch).truncate()).on_hover_text(tr("Choose a sound in the library")).clicked() {
                    open_library = true;
                }
                }
                ui.spacing_mut().slider_width = 70.0;
                ui.add(egui::Slider::new(&mut track.volume, 0.0..=1.5).show_value(false)).on_hover_text(tr("Volume"));
                ui.add(egui::DragValue::new(&mut track.pan).range(-1.0..=1.0).speed(0.02).fixed_decimals(1))
                    .on_hover_text(tr("Pan (-1 left, 1 right)"));
            });
        });
        if make_lead {
            for (k, track) in self.project.tracks.iter_mut().enumerate() {
                track.lead = k == ti;
            }
        }
        let resp = inner.response.interact(Sense::click());
        if resp.clicked() || open_library {
            // With Ctrl, the track joins the others in the editor instead.
            if resp.clicked() && !open_library && ui.input(|i| i.modifiers.command) {
                self.toggle_editor_track(ti);
                self.show_bottom = true;
                self.bottom_tab = crate::app::BottomTab::Editor;
            } else {
                self.editor_tracks.clear();
                self.select_track(ti);
            }
        }
        if open_library {
            self.reveal_patch(ti);
        }
        resp.context_menu(|ui| {
            self.selected_track = ti;
            let actions = [
                (tr("Duplicate track"), TrackAction::Duplicate),
                (tr("Move up"), TrackAction::Move(-1)),
                (tr("Move down"), TrackAction::Move(1)),
                (tr("Delete track"), TrackAction::Delete),
            ];
            for (label, action) in actions {
                if action == TrackAction::Delete {
                    ui.separator();
                }
                if ui.button(label).clicked() {
                    self.pending_track = Some((ti, action));
                    ui.close_menu();
                }
            }
            ui.separator();
            let Some(track) = self.project.tracks.get(ti) else { return };
            let (output, input, audio) = (track.output.clone(), track.input.clone(), track.kind.is_audio());
            ui.menu_button(tr("Audio output of this track"), |ui| {
                let chosen = radio_list(ui, &output, device_items(&self.audio_outputs, tr("Main mix")));
                if !output.is_empty() && !self.audio_outputs.contains(&output) {
                    ui.weak(tf!("“{}” is not on this computer: the track plays in the main mix.", output));
                }
                if let Some(name) = chosen {
                    self.set_track_output(ti, &name);
                    ui.close_menu();
                }
            });
            if audio {
                ui.menu_button(tr("Audio input of this track"), |ui| {
                    let chosen = radio_list(ui, &input, device_items(&self.audio_inputs, tr("The input chosen in the options")));
                    ui.separator();
                    ui.weak(tr("Channel of the input"));
                    let own = self.project.tracks.get(ti).map_or(0, |t| t.input_channel);
                    let channel = radio_list(ui, &own, channel_items(crate::audio::input_channel_count(&self.input_for(ti)), tr("As chosen in the Audio menu")));
                    if let Some(number) = channel {
                        self.set_track_input_channel(ti, number);
                        ui.close_menu();
                    }
                    if let Some(name) = chosen {
                        self.set_track_input(ti, &name);
                        ui.close_menu();
                    }
                });
            }
        });
    }

    /// Longest an audio region can be before its clip runs out, in beats.
    fn audio_max_len(&self, region: &Region) -> Option<f64> {
        let Content::Audio(audio) = &region.content else { return None };
        let clip = self.clips.get(&audio.clip)?;
        let seconds = (clip.seconds() - audio.offset).max(0.05);
        Some(seconds * self.project.tempo_at(region.start) / 60.0)
    }

    fn draw_region(&self, painter: &Painter, lane: Rect, ti: usize, ri: usize, dim: bool) {
        let track = &self.project.tracks[ti];
        let region = &track.regions[ri];
        let body = Rect::from_min_max(
            pos2(self.x_of(lane, region.start), lane.top() + 3.0),
            pos2(self.x_of(lane, region.end()), lane.bottom() - 3.0),
        );
        if !body.intersects(lane) {
            return;
        }
        let selected = self.selected_region == Some((ti, ri));
        let mut color = track_color(track.color);
        if dim {
            color = color.gamma_multiply(0.35);
        } else if !selected {
            color = color.gamma_multiply(0.75);
        }
        painter.rect_filled(body, 5.0, color);
        if selected && !dim {
            painter.rect_stroke(body, 5.0, stroke(1.5, crate::theme::current().outline()));
        }
        let clip = painter.with_clip_rect(body.intersect(lane));
        let ink = Color32::from_gray(25);
        clip.text(body.left_top() + vec2(5.0, 2.0), Align2::LEFT_TOP, &region.name, FontId::proportional(11.0), ink);

        let cycle = region.cycle_len().min(region.length.max(0.0625));
        // Notches where each repeat starts.
        if region.loop_len.is_some() {
            let mut at = region.start + cycle;
            while at < region.end() - 1e-9 {
                let x = self.x_of(lane, at);
                clip.line_segment([pos2(x, body.top()), pos2(x, body.top() + 9.0)], stroke(2.0, ink));
                clip.line_segment([pos2(x, body.top() + 9.0), pos2(x, body.bottom())], stroke(1.0, ink.gamma_multiply(0.35)));
                at += cycle;
            }
        }
        let inner = Rect::from_min_max(pos2(body.left(), body.top() + 15.0), pos2(body.right(), body.bottom() - 4.0));
        let generated;
        let notes: &[Note] = match &region.content {
            Content::Midi(notes) => notes,
            Content::Drummer(params) => {
                generated = self.project.drummer_notes(params, region.start, cycle);
                &generated
            }
            Content::Audio(audio) => {
                self.draw_waveform(&clip, lane, inner, region, audio.clip, audio.offset, audio.reverse);
                return;
            }
        };
        let low = notes.iter().map(|n| n.pitch).min().unwrap_or(60);
        let high = notes.iter().map(|n| n.pitch).max().unwrap_or(60);
        let span = f32::from(high - low).max(12.0);
        let repeats = ((region.length / cycle).ceil() as usize).clamp(1, 512);
        for k in 0..repeats {
            let base = region.start + k as f64 * cycle;
            for note in notes {
                let on = base + note.start;
                if note.start >= cycle || on >= region.end() {
                    continue;
                }
                let y = inner.bottom() - f32::from(note.pitch - low) / span * inner.height();
                let x0 = self.x_of(lane, on);
                let x1 = self.x_of(lane, (on + note.length).min(region.end())).max(x0 + 1.5);
                clip.line_segment([pos2(x0, y), pos2(x1, y)], stroke(2.0, ink));
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_waveform(&self, painter: &Painter, lane: Rect, inner: Rect, region: &Region, clip: u64, offset: f64, reverse: bool) {
        let ink = Color32::from_gray(25);
        let (Some(peaks), Some(data)) = (self.waveforms.get(&clip), self.clips.get(&clip)) else {
            painter.text(inner.left_center() + vec2(6.0, 0.0), Align2::LEFT_CENTER, tr("audio file not found"), FontId::proportional(11.0), ink);
            return;
        };
        let total = data.seconds().max(1e-6);
        let start_sec = self.project.beat_to_sec(region.start);
        let cycle_sec = region.loop_len.map(|len| self.project.beat_to_sec(region.start + len) - start_sec);
        let mid = inner.center().y;
        let half = inner.height() / 2.0;
        let from = inner.left().max(lane.left()) as i32;
        let to = inner.right().min(lane.right()) as i32;
        for px in from..to {
            let beat = self.beat_of(lane, px as f32);
            let mut t = self.project.beat_to_sec(beat) - start_sec;
            if let Some(cycle) = cycle_sec.filter(|c| *c > 1e-6) {
                t %= cycle;
            }
            let mut at = (offset + t) / total;
            if reverse {
                at = 1.0 - at;
            }
            if !(0.0..1.0).contains(&at) {
                continue;
            }
            let (lo, hi) = peaks[((at * peaks.len() as f64) as usize).min(peaks.len() - 1)];
            let x = px as f32 + 0.5;
            painter.line_segment([pos2(x, mid - hi.clamp(-1.0, 1.0) * half), pos2(x, mid - lo.clamp(-1.0, 1.0) * half + 0.5)], stroke(1.0, ink));
        }
    }

    fn lane(&mut self, ui: &mut egui::Ui, ti: usize) {
        if ti >= self.project.tracks.len() {
            return;
        }
        let (resp, painter) = ui.allocate_painter(vec2(ui.available_width(), LANE_H), Sense::click_and_drag());
        let rect = resp.rect;
        self.lane_w = rect.width();
        self.lane_rects.push(rect);
        let painter = painter.with_clip_rect(rect);
        self.draw_grid(ui, &painter, rect);
        let automation = self.automation;
        for ri in 0..self.project.tracks[ti].regions.len() {
            self.draw_region(&painter, rect, ti, ri, automation.is_some());
        }
        self.draw_playhead(ui, &painter, rect);

        if let Some(param) = automation {
            let track = &self.project.tracks[ti];
            let (id, spec) = match param {
                AutoParam::Volume => (
                    CurveId::Volume(ti),
                    LaneSpec { min: 0.0, max: 1.5, default: f64::from(track.volume), step: false, round: 0.0, color: crate::theme::current().outline(), unit: tr("(volume)") },
                ),
                AutoParam::Pan => (
                    CurveId::Pan(ti),
                    LaneSpec { min: -1.0, max: 1.0, default: f64::from(track.pan), step: false, round: 0.0, color: crate::theme::current().outline(), unit: tr("(pan)") },
                ),
                AutoParam::Fx(fx) => {
                    let (min, max) = fx.range();
                    (CurveId::Fx(ti, fx), LaneSpec { min, max, default: fx.get(&track.fx), step: false, round: 0.0, color: crate::theme::current().outline(), unit: tr(fx.label()) })
                }
            };
            if resp.clicked() {
                self.selected_track = ti;
            }
            if !track.locked {
                self.point_lane(ui, &resp, &painter, rect, id, &spec);
            }
            return;
        }

        let locked = self.project.tracks[ti].locked;
        let kind = self.project.tracks[ti].kind;
        let bar_len = self.project.bar_len();
        // Which zone of a region is under a point: its body, or its right edge
        // (upper half loops, lower half resizes).
        let zone = |app: &App, p: Pos2| -> Option<(usize, u8)> {
            let ri = app.hit_region(ti, app.beat_of(rect, p.x))?;
            let region = &app.project.tracks[ti].regions[ri];
            let near_end = app.x_of(rect, region.end()) - p.x <= EDGE;
            Some((ri, if !near_end { 0 } else if p.y < rect.center().y { 1 } else { 2 }))
        };

        if let (Some(p), false) = (resp.hover_pos(), locked) {
            match zone(self, p) {
                Some((_, 1)) => {
                    ui.ctx().set_cursor_icon(CursorIcon::Alias);
                    resp.clone().on_hover_text_at_pointer(tr("Drag to loop"));
                }
                Some((_, 2)) => ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal),
                Some((_, _)) => {
                    resp.clone().on_hover_text_at_pointer(tr("Drag to move · with Alt or Ctrl held: a copy"));
                }
                _ => {}
            }
        }

        if resp.drag_started() {
            if let Some(p) = ui.input(|i| i.pointer.press_origin()) {
                self.selected_track = ti;
                if let Some((ri, zone)) = zone(self, p) {
                    self.select_region(ti, ri);
                    if !locked {
                        let grab = self.beat_of(rect, p.x) - self.project.tracks[ti].regions[ri].start;
                        self.checkpoint();
                        // Dragged with Alt or Ctrl held: a copy is what moves.
                        let copy = zone == 0 && ui.input(|i| i.modifiers.alt || i.modifiers.ctrl);
                        let ri = if copy {
                            let twin = self.project.tracks[ti].regions[ri].clone();
                            self.project.tracks[ti].regions.push(twin);
                            let new = self.project.tracks[ti].regions.len() - 1;
                            self.select_region(ti, new);
                            new
                        } else {
                            ri
                        };
                        self.drag = Some(match zone {
                            1 => Drag::LoopRegion { ti, ri },
                            2 => Drag::ResizeRegion { ti, ri },
                            _ => Drag::MoveRegion { ti, ri, grab },
                        });
                    }
                }
            }
        }

        if let Some(p) = resp.interact_pointer_pos() {
            let beat = self.beat_of(rect, p.x);
            let hit = self.hit_region(ti, beat);
            if resp.double_clicked() && hit.is_none() && !locked {
                let start = (beat / bar_len).floor().max(0.0) * bar_len;
                match kind {
                    TrackKind::Instrument => {
                        self.checkpoint();
                        let name = tf!("Region {}", self.project.tracks[ti].regions.len() + 1);
                        self.project.tracks[ti].regions.push(Region::new(name, start, bar_len * 4.0));
                        self.select_region(ti, self.project.tracks[ti].regions.len() - 1);
                    }
                    TrackKind::Drummer => {
                        self.checkpoint();
                        // Carry on with the settings of the track's last drummer region.
                        let params = self.project.tracks[ti]
                            .regions
                            .iter()
                            .rev()
                            .find_map(|r| match &r.content {
                                Content::Drummer(p) => Some(*p),
                                _ => None,
                            })
                            .unwrap_or_else(DrummerParams::default);
                        let mut region = Region::new(tr("Drums"), start, bar_len * 8.0);
                        region.content = Content::Drummer(params);
                        self.project.tracks[ti].regions.push(region);
                        self.select_region(ti, self.project.tracks[ti].regions.len() - 1);
                    }
                    TrackKind::Audio | TrackKind::Guitar => {
                        self.status = tr("Audio track: record (R) or import a file (File menu).").to_owned();
                    }
                    TrackKind::Video => {}
                }
            } else if resp.clicked() || resp.secondary_clicked() {
                self.selected_track = ti;
                match hit {
                    Some(ri) => self.select_region(ti, ri),
                    None => {
                        self.selected_region = None;
                        self.selected_notes.clear();
                    }
                }
            }
        }

        resp.context_menu(|ui| {
            let Some((sti, _)) = self.selection().filter(|s| s.0 == ti) else {
                if ui.add_enabled(self.clipboard.is_some(), egui::Button::new(tr("Paste at the playhead"))).clicked() {
                    self.paste_region();
                    ui.close_menu();
                }
                return;
            };
            let looped = self.selection().and_then(|(t, r)| self.region(t, r)).is_some_and(|r| r.loop_len.is_some());
            // The entries of the menu, each with what it does.
            type Entry<'a> = (&'a str, fn(&mut App));
            let actions: [Entry; 6] = [
                (tr("Cut"), App::cut_region),
                (tr("Copy"), App::copy_region),
                (tr("Split at the playhead"), App::split_region),
                (tr("Join with the next region"), App::join_regions),
                (if looped { tr("Stop looping") } else { tr("Loop region") }, App::toggle_loop),
                (tr("Delete"), App::delete_selected_region),
            ];
            for (label, action) in actions {
                if ui.add_enabled(!self.track_locked(sti), egui::Button::new(label)).clicked() {
                    action(self);
                    ui.close_menu();
                }
            }
        });
    }

    /// Moves, resizes or loops the region being dragged, wherever the pointer is.
    fn apply_region_drag(&mut self, ctx: &egui::Context) {
        let drag = match &self.drag {
            Some(drag @ (Drag::MoveRegion { .. } | Drag::ResizeRegion { .. } | Drag::LoopRegion { .. })) => drag.clone(),
            _ => return,
        };
        let (pos, down) = ctx.input(|i| (i.pointer.latest_pos(), i.pointer.primary_down()));
        if !down {
            self.drag = None;
            return;
        }
        let Some(p) = pos else { return };
        let grid = self.region_grid();
        match drag {
            Drag::MoveRegion { ti, ri, grab } => {
                let Some(rect) = self.lane_rects.get(ti).copied() else { return };
                let beat = self.beat_of(rect, p.x);
                let Some(region) = self.project.tracks.get_mut(ti).and_then(|t| t.regions.get_mut(ri)) else { return };
                region.start = snap(beat - grab, grid).max(0.0);
                // Dropping onto another track of the right kind moves the region there.
                let over = self.lane_rects.iter().position(|r| p.y >= r.top() && p.y < r.bottom());
                if let Some(tj) = over.filter(|tj| *tj != ti) {
                    let content = &self.project.tracks[ti].regions[ri].content;
                    let target = &self.project.tracks[tj];
                    if accepts(target.kind, content) && !target.locked {
                        let region = self.project.tracks[ti].regions.remove(ri);
                        self.project.tracks[tj].regions.push(region);
                        let new = self.project.tracks[tj].regions.len() - 1;
                        self.drag = Some(Drag::MoveRegion { ti: tj, ri: new, grab });
                        self.selected_region = Some((tj, new));
                        self.selected_track = tj;
                    }
                }
            }
            Drag::ResizeRegion { ti, ri } => {
                let Some(rect) = self.lane_rects.get(ti).copied() else { return };
                let beat = self.beat_of(rect, p.x);
                let limit = self.region(ti, ri).filter(|r| r.loop_len.is_none()).and_then(|r| self.audio_max_len(r));
                let Some(region) = self.project.tracks.get_mut(ti).and_then(|t| t.regions.get_mut(ri)) else { return };
                let mut length = (snap(beat, grid) - region.start).max(grid);
                if let Some(limit) = limit {
                    length = length.min(limit);
                }
                region.length = length;
                if region.loop_len.is_some_and(|cycle| length <= cycle) {
                    region.loop_len = None;
                }
            }
            Drag::LoopRegion { ti, ri } => {
                let Some(rect) = self.lane_rects.get(ti).copied() else { return };
                let beat = self.beat_of(rect, p.x);
                let Some(region) = self.project.tracks.get_mut(ti).and_then(|t| t.regions.get_mut(ri)) else { return };
                let cycle = region.loop_len.unwrap_or(region.length);
                let length = snap(beat, grid) - region.start;
                if length > cycle + 1e-9 {
                    region.loop_len = Some(cycle);
                    region.length = length;
                } else {
                    region.loop_len = None;
                    region.length = cycle;
                }
            }
            _ => {}
        }
    }
}
