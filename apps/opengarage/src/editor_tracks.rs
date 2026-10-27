//! Several tracks in the editor: the ones chosen with Ctrl on their headers,
//! shown one under the other, or, with auto-switch on, one at a time: the
//! one being played, the first chosen having priority.

use og_engine::Content;

use crate::app::{App, SNAP};

impl App {
    /// Bars of silence before auto-switch moves on: the user's choice, or two.
    pub fn auto_switch_bars(&self) -> u32 {
        if self.settings.auto_switch_bars == 0 {
            2
        } else {
            self.settings.auto_switch_bars
        }
    }

    /// The tracks the editor is for, in the order they were chosen: the
    /// selected track alone, unless others were added to it with Ctrl.
    pub fn editor_track_list(&mut self) -> Vec<usize> {
        let count = self.project.tracks.len();
        self.editor_tracks.retain(|t| *t < count);
        if !self.editor_tracks.contains(&self.selected_track) {
            self.editor_tracks = vec![self.selected_track];
        }
        self.editor_tracks.clone()
    }

    /// A track was deleted: it leaves the editor, the others keep their
    /// place in it, and none takes its place. When it was the selected one,
    /// the first of the others is selected. The selected track has already
    /// been renumbered by the caller.
    pub fn editor_track_deleted(&mut self, ti: usize, was_selected: bool) {
        let several = self.editor_tracks.len() > 1 && self.editor_tracks.contains(&ti);
        self.beat_selection = self.beat_selection.filter(|s| s.track != ti).map(|mut s| {
            s.track -= usize::from(s.track > ti);
            s
        });
        self.editor_tracks.retain(|t| *t != ti);
        for t in &mut self.editor_tracks {
            if *t > ti {
                *t -= 1;
            }
        }
        self.followed = self.followed.filter(|f| *f != ti).map(|f| if f > ti { f - 1 } else { f });
        if several {
            if was_selected {
                if let Some(&first) = self.editor_tracks.first() {
                    let kept = self.editor_tracks.clone();
                    self.select_track(first);
                    self.editor_tracks = kept;
                }
            }
        } else {
            self.editor_tracks.clear();
        }
    }

    /// A track was inserted at `ti`: the tracks after it move one place on.
    pub fn editor_track_inserted(&mut self, ti: usize) {
        let chosen = self.beat_selection.as_mut().map(|s| &mut s.track);
        for t in self.editor_tracks.iter_mut().chain(self.followed.as_mut()).chain(chosen) {
            if *t >= ti {
                *t += 1;
            }
        }
    }

    /// Two tracks swapped places: the editor keeps showing the same tracks.
    pub fn editor_tracks_swapped(&mut self, a: usize, b: usize) {
        let chosen = self.beat_selection.as_mut().map(|s| &mut s.track);
        for t in self.editor_tracks.iter_mut().chain(self.followed.as_mut()).chain(chosen) {
            if *t == a {
                *t = b;
            } else if *t == b {
                *t = a;
            }
        }
    }

    /// Ctrl and a click on a header: the track joins the editor, or leaves
    /// it. The one that leaves, if it was the selected track, gives way to
    /// the first of the others.
    pub fn toggle_editor_track(&mut self, ti: usize) {
        if ti >= self.project.tracks.len() {
            return;
        }
        let mut tracks = self.editor_track_list();
        match tracks.iter().position(|t| *t == ti) {
            Some(at) if tracks.len() > 1 => {
                tracks.remove(at);
                self.editor_tracks = tracks;
                if self.selected_track == ti {
                    let first = self.editor_tracks[0];
                    self.select_track(first);
                }
            }
            Some(_) => {}
            None => self.editor_tracks.push(ti),
        }
    }

    /// Whether something of a track sounds at `beat` of the song: a note of
    /// a region of notes, or any region of sound or of the drummer.
    pub fn track_plays_at(&self, ti: usize, beat: f64) -> bool {
        let Some(track) = self.project.tracks.get(ti) else { return false };
        track.regions.iter().filter(|r| r.start <= beat && beat < r.end()).any(|r| match &r.content {
            Content::Midi(notes) => {
                let local = (beat - r.start) % r.cycle_len().max(SNAP);
                notes.iter().any(|n| n.start <= local && local < n.start + n.length)
            }
            _ => true,
        })
    }

    /// Whether a note of a track starts after `beat` and within `horizon`
    /// beats of it: what is still to come on the track. Any region of sound
    /// or of the drummer that reaches into that stretch counts.
    pub fn track_plays_within(&self, ti: usize, beat: f64, horizon: f64) -> bool {
        let Some(track) = self.project.tracks.get(ti) else { return false };
        let until = beat + horizon;
        track.regions.iter().filter(|r| r.start < until && r.end() > beat).any(|r| {
            let Content::Midi(notes) = &r.content else { return r.end() > beat + 1e-6 };
            let cycle = r.cycle_len().max(SNAP);
            // The stretch in the region's own time, across its passes.
            let (from, to) = ((beat - r.start).max(-1e-9), (until - r.start).min(r.length));
            let mut pass = (from / cycle).floor().max(0.0) * cycle;
            while pass < to {
                if notes.iter().any(|n| pass + n.start > from && pass + n.start <= to) {
                    return true;
                }
                pass += cycle;
            }
            false
        })
    }

    /// With auto-switch on: the track to show among `tracks`. The one
    /// followed so far, until nothing more comes on it within the chosen
    /// bars: then, as its last note is struck, the next track that sounds,
    /// or else the next with a note to come. The first chosen takes the
    /// place back whenever it sounds with more to come.
    fn follow(&mut self, tracks: &[usize]) -> usize {
        let first = tracks[0];
        let mut followed = self.followed.filter(|t| tracks.contains(t)).unwrap_or(first);
        if self.playing {
            let beat = self.position;
            let horizon = f64::from(self.auto_switch_bars()) * self.project.bar_len();
            // Back to the first chosen when it sounds and goes on; the one
            // followed is left as its last note is struck.
            if followed != first && self.track_plays_at(first, beat) && self.track_plays_within(first, beat, horizon) {
                followed = first;
            } else if !self.track_plays_within(followed, beat, horizon) {
                let at = tracks.iter().position(|t| *t == followed).unwrap_or(0);
                let others: Vec<usize> = (1..tracks.len()).map(|k| tracks[(at + k) % tracks.len()]).collect();
                let next = others.iter().find(|t| self.track_plays_at(**t, beat)).or_else(|| others.iter().find(|t| self.track_plays_within(**t, beat, horizon)));
                if let Some(next) = next {
                    followed = *next;
                }
            }
        }
        self.followed = Some(followed);
        followed
    }

    /// The region of a track the editor shows when the track is not the
    /// selected one: the one under the playhead, or else its first.
    fn region_shown(&self, ti: usize) -> Option<usize> {
        let track = self.project.tracks.get(ti)?;
        let position = self.position;
        let under = track.regions.iter().position(|r| r.start <= position && position < r.end());
        under.or_else(|| (0..track.regions.len()).min_by(|a, b| track.regions[*a].start.total_cmp(&track.regions[*b].start)))
    }

    /// What the editor shows: a region of each track, and whether several
    /// tracks were chosen for it. The selected track shows its selected
    /// region; the others the one under the playhead.
    pub fn editor_shown(&mut self) -> (Vec<(usize, usize)>, bool) {
        let tracks = self.editor_track_list();
        let primary = self.selection();
        if tracks.len() < 2 {
            self.followed = None;
            return (primary.into_iter().collect(), false);
        }
        let shown: Vec<usize> = if self.auto_switch {
            vec![self.follow(&tracks)]
        } else {
            self.followed = None;
            tracks
        };
        let regions = shown
            .into_iter()
            .filter_map(|ti| match primary {
                Some((pt, pr)) if pt == ti => Some((ti, pr)),
                _ => self.region_shown(ti).map(|ri| (ti, ri)),
            })
            .collect();
        (regions, true)
    }
}
