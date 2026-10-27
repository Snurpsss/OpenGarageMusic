//! Beats chosen in the score or the tablature, with Shift and a click, to be
//! played in a loop: the loop area of the song is set to them.

use eframe::egui::{self, pos2, Color32, Rect};

use crate::app::{App, SNAP};
use crate::ui_bottom::Lines;

/// Beats of a region chosen in its editor, in the region's own time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeatSelection {
    pub track: usize,
    pub region: usize,
    /// The beat chosen first, which the next one is counted from.
    pub anchor: f64,
    pub from: f64,
    pub to: f64,
    /// Whether the second beat was chosen: the next click starts again.
    pub complete: bool,
}

impl App {
    /// The length of the pattern a region repeats, which its editor shows.
    fn shown_cycle(&self, ti: usize, ri: usize) -> Option<f64> {
        let region = self.project.tracks.get(ti)?.regions.get(ri)?;
        Some(region.cycle_len().min(region.length).max(SNAP))
    }

    /// The beats chosen in the editor of a region, from and to.
    pub fn beats_selected(&self, ti: usize, ri: usize) -> Option<(f64, f64)> {
        let selection = self.beat_selection.filter(|s| s.track == ti && s.region == ri)?;
        let cycle = self.shown_cycle(ti, ri)?;
        (selection.from < cycle).then(|| (selection.from, selection.to.min(cycle)))
    }

    /// What Shift and a click on `beat` would choose: from the first beat
    /// chosen to this one when a second is awaited, this one alone otherwise.
    pub fn beats_to_choose(&self, ti: usize, ri: usize, beat: f64) -> (f64, f64) {
        let beat = beat.floor();
        match self.beat_selection.filter(|s| s.track == ti && s.region == ri && !s.complete) {
            Some(s) => (s.anchor.min(beat), s.anchor.max(beat) + 1.0),
            None => (beat, beat + 1.0),
        }
    }

    /// Shift and a click on `beat` of a region's editor: the first beat, then
    /// the last, then a new first.
    pub fn choose_beat(&mut self, ti: usize, ri: usize, beat: f64) {
        let Some(cycle) = self.shown_cycle(ti, ri) else { return };
        if beat < 0.0 || beat >= cycle {
            return;
        }
        let (from, to) = self.beats_to_choose(ti, ri, beat);
        let second = self.beat_selection.is_some_and(|s| s.track == ti && s.region == ri && !s.complete);
        let anchor = if second { self.beat_selection.map_or(from, |s| s.anchor) } else { beat.floor() };
        self.beat_selection = Some(BeatSelection { track: ti, region: ri, anchor, from, to: to.min(cycle), complete: second });
    }

    /// Where the chosen beats are in the song: in the pass of the region
    /// being played, or else in its first.
    fn beats_in_song(&self, ti: usize, ri: usize) -> Option<(f64, f64)> {
        let (from, to) = self.beats_selected(ti, ri)?;
        let region = &self.project.tracks[ti].regions[ri];
        let cycle = self.shown_cycle(ti, ri)?;
        let inside = self.position - region.start;
        let pass = if inside >= 0.0 && inside < region.length { (inside / cycle).floor() } else { 0.0 };
        let start = region.start + pass * cycle;
        Some((start + from, (start + to).min(region.end())))
    }

    /// Whether the song loops over the beats chosen in a region.
    pub fn looping_beats(&self, ti: usize, ri: usize) -> bool {
        let project = &self.project;
        project.cycle_enabled
            && self.beats_selected(ti, ri).is_some_and(|(from, to)| {
                let region = &project.tracks[ti].regions[ri];
                let cycle = region.cycle_len().min(region.length).max(SNAP);
                let offset = (project.cycle_start - region.start - from).rem_euclid(cycle);
                (offset < 1e-6 || cycle - offset < 1e-6) && ((project.cycle_end - project.cycle_start) - (to - from)).abs() < 1e-6
            })
    }

    /// The button of the editor: the song loops over the chosen beats, from
    /// their first, and plays; or, when it already does, stops looping.
    pub fn toggle_loop_beats(&mut self, ti: usize, ri: usize) {
        if self.looping_beats(ti, ri) {
            self.project.cycle_enabled = false;
            return;
        }
        let Some((start, end)) = self.beats_in_song(ti, ri) else { return };
        self.project.cycle_start = start;
        self.project.cycle_end = end;
        self.project.cycle_enabled = true;
        self.go_to(start);
        if !self.playing {
            self.toggle_play();
            self.playing = true;
        }
    }

    /// The instant of a region's editor under `p`, in beats from the start of
    /// the region, when it is on a line: where a click moves the playhead.
    pub fn instant_at(lines: &Lines, rect: Rect, left: f32, line: f32, cycle: f64, p: egui::Pos2) -> Option<f64> {
        let system = lines.systems.iter().find(|s| p.y >= rect.top() + s.top && p.y < rect.top() + s.top + line)?;
        let beat = system.from + f64::from(p.x - left).max(0.0) / lines.ppb;
        (beat < cycle.min(system.to)).then_some(beat)
    }

    /// The beat of a region's editor under `p`, when it is on a line.
    pub fn beat_at(lines: &Lines, rect: Rect, left: f32, line: f32, cycle: f64, p: egui::Pos2) -> Option<f64> {
        let system = lines.systems.iter().find(|s| p.y >= rect.top() + s.top && p.y < rect.top() + s.top + line)?;
        if p.x < left {
            return None;
        }
        let beat = (system.from + f64::from(p.x - left) / lines.ppb).floor();
        (beat >= 0.0 && beat < cycle.min(system.to)).then_some(beat)
    }

    /// The chosen beats, and with Shift held the ones a click would choose,
    /// shaded on each line they are in. Gives the beat under the pointer.
    #[allow(clippy::too_many_arguments)]
    pub fn shade_beats(&self, ui: &egui::Ui, painter: &egui::Painter, resp: &egui::Response, rect: Rect, lines: &Lines, left: f32, line: f32, cycle: f64, ti: usize, ri: usize) -> Option<f64> {
        let hovered = resp.hover_pos().and_then(|p| Self::beat_at(lines, rect, left, line, cycle, p));
        let shift = ui.input(|i| i.modifiers.shift);
        let chosen = ui.visuals().selection.bg_fill;
        let shade = |(from, to): (f64, f64), color: Color32| {
            for system in &lines.systems {
                let (a, b) = (from.max(system.from), to.min(system.to).min(cycle));
                if b <= a {
                    continue;
                }
                let x = |beat: f64| left + ((beat - system.from) * lines.ppb) as f32;
                let top = rect.top() + system.top;
                painter.rect_filled(Rect::from_min_max(pos2(x(a), top), pos2(x(b), top + line)), 0.0, color);
            }
        };
        if let Some(selected) = self.beats_selected(ti, ri) {
            shade(selected, chosen.gamma_multiply(0.45));
        }
        if let (true, Some(beat)) = (shift, hovered) {
            shade(self.beats_to_choose(ti, ri, beat), chosen.gamma_multiply(0.3));
        }
        hovered
    }
}
