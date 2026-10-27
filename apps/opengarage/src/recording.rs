//! The recordings of the song in a project, and playing the song with its
//! original sound.
//!
//! A recording is a track: an audio track holding a file of the user's, or
//! a video track holding a YouTube video a tablature site gave, each with
//! where in it the beats of the score fall (its timing). A file is found in
//! step with the score by `og_engine::align`, in the background; a video's
//! timing comes from the site. The song is then played either as written,
//! at its own tempo, the recordings silent, or with its original sound, at
//! the pace of the recording that leads (see `og_engine::recording`).

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use eframe::egui;
use og_engine::align::{self, Alignment};
use og_engine::recording::{beat_at, is_recording, lead, sec_at};
use og_engine::{codec, templates, AudioRef, ClipData, Content, Region, Timing, TrackKind, VideoChoice};

use crate::app::App;
use crate::files;
use crate::i18n::tr;
use crate::sites::VideoOffer;
use crate::video::{VideoPlayer, BUFFERING, PLAYING};

/// A recording read and found in step with the score.
pub struct Linked {
    pub path: PathBuf,
    pub data: ClipData,
    pub alignment: Alignment,
}

/// How far the song may be from the video it follows before it is moved to
/// it, and how long it is then left before it is moved again.
const FOLLOW_DRIFT: f64 = 0.12;
const FOLLOW_SETTLE: Duration = Duration::from_millis(1000);
/// How long the video may keep the song waiting (an advertisement, a slow
/// connection) before the song stops for it.
const PATIENCE: Duration = Duration::from_millis(1500);

impl App {
    /// Whether the project has a recording to play the song with.
    pub fn has_recording(&self) -> bool {
        lead(&self.project).is_some()
    }

    /// Plays the song with its original sound, or as written.
    pub fn set_follow(&mut self, on: bool) {
        let on = on && self.has_recording();
        if self.project.follow != on {
            self.project.follow = on;
            self.status = if on {
                tr("Played with the original sound: at the pace of the recording, the other tracks silent unless you turn them on.").to_owned()
            } else {
                tr("Played as written: at the tempo of the score, the recordings silent.").to_owned()
            };
        }
    }

    /// Asks for a recording of the song and finds it in step with the score.
    pub fn link_recording_dialog(&mut self) {
        let Some(path) = files::audio_file() else { return };
        self.link_recording(path);
    }

    /// Reads a recording of the song and finds where the beats of the score
    /// fall in it, in the background; `poll_recording` takes the result.
    pub fn link_recording(&mut self, path: PathBuf) {
        let project = self.project.clone();
        let (send, receive) = mpsc::channel();
        std::thread::spawn(move || {
            let result = codec::decode_file(&path).and_then(|data| {
                let alignment = align::align(&project, &data.samples, usize::from(data.channels), data.sample_rate).ok_or_else(|| tr("the score has no notes to compare").to_owned())?;
                Ok(Linked { path, data, alignment })
            });
            let _ = send.send(result);
        });
        self.aligning = Some(receive);
        self.status = tr("Finding the recording in step with the score…").to_owned();
    }

    /// Takes a recording found in step, once it is.
    pub fn poll_recording(&mut self) {
        let Some(result) = crate::app::take_result(&mut self.aligning, tr("the reading stopped unexpectedly")) else { return };
        match result {
            Ok(linked) => self.add_recording_track(linked),
            Err(e) => self.status = tf!("The recording was not read: {e}" [e]),
        }
    }

    /// Makes the recording that leads the one at `index`, the others not.
    fn lead_with(&mut self, index: usize) {
        for (k, track) in self.project.tracks.iter_mut().enumerate() {
            track.lead = k == index;
        }
    }

    /// Puts a recording found in step on a track of its own, which leads, and
    /// plays the song with it.
    fn add_recording_track(&mut self, linked: Linked) {
        self.checkpoint();
        let name = linked.path.file_stem().map_or_else(|| tr("Recording").to_owned(), |s| s.to_string_lossy().into_owned());
        let seconds = linked.data.seconds();
        let id = self.project.next_clip_id();
        let path = linked.path.clone();
        self.add_clip(id, &name, &path, linked.data);
        let mut track = templates::track(&name, TrackKind::Audio, "No effect", 5);
        track.timing = linked.alignment.timing.clone();
        let end = beat_at(&track.timing, seconds).unwrap_or(self.project.end_beat()).max(self.project.bar_len());
        let mut region = Region::new(name.clone(), 0.0, end);
        region.content = Content::Audio(AudioRef::new(id));
        track.regions.push(region);
        self.project.tracks.push(track);
        let index = self.project.tracks.len() - 1;
        // The words beside the recording, when the song has none yet.
        if self.project.lyrics.is_empty() {
            self.words_beside_recording();
        }
        self.lead_with(index);
        self.project.follow = true;
        let agreement = (linked.alignment.agreement * 100.0).round();
        self.status = if linked.alignment.agreement >= align::FAIR {
            tf!("“{name}” is in step with the score ({agreement} % alike). The song now plays with it; the other tracks are silent until you turn them on." [name, agreement])
        } else {
            tf!("“{name}” is barely like the score ({agreement} % alike): is it the same song, the same version? It plays with the song all the same; check it, and shift it if need be." [name, agreement])
        };
    }

    /// Adds the videos a site gave for the song, in step with the score, as a
    /// video track that plays one of them, the original one when the site says
    /// which, and keeps the others to be chosen: the second each bar starts at
    /// in a video, the bars starting at `bars` (in beats, in the order they are
    /// played).
    pub fn add_video_track(&mut self, offers: &[VideoOffer], bars: &[f64]) {
        let offers: Vec<&VideoOffer> = offers.iter().filter(|offer| offer.points.len().min(bars.len()) >= 2).collect();
        let Some(offer) = offers.iter().find(|v| v.label.is_empty() || v.label == "original").or(offers.first()).copied() else { return };
        let timing = |offer: &VideoOffer| bars.iter().zip(&offer.points).map(|(&beat, &sec)| Timing { beat, sec }).collect::<Vec<_>>();
        let mut track = templates::track("YouTube", TrackKind::Video, "", 6);
        track.video = offer.id.clone();
        track.timing = timing(offer);
        if offers.len() > 1 {
            track.videos = offers.iter().map(|offer| VideoChoice { id: offer.id.clone(), label: offer.label.clone(), timing: timing(offer) }).collect();
        }
        let mut region = Region::new("YouTube", 0.0, self.project.end_beat());
        region.content = Content::Midi(Vec::new());
        track.regions.push(region);
        self.project.tracks.push(track);
        let index = self.project.tracks.len() - 1;
        if !self.project.tracks.iter().any(|t| t.lead && t.kind == TrackKind::Audio) {
            self.lead_with(index);
        }
        if offer.points.len().abs_diff(bars.len()) > 1 {
            let (points, count) = (offer.points.len(), bars.len());
            self.status = tf!("The video gives {points} bars where the tablature has {count}: it may drift; shift it if need be." [points, count]);
        }
    }

    /// Moves a recording against the score, by so many seconds (later when positive).
    pub fn shift_recording(&mut self, ti: usize, seconds: f64) {
        if let Some(track) = self.project.tracks.get_mut(ti) {
            for point in &mut track.timing {
                point.sec += seconds;
            }
        }
    }

    /// Plays another of the videos the site gave, on a video track. The video
    /// left keeps its timing, shifts included, for coming back to it.
    pub fn choose_video(&mut self, ti: usize, choice: usize) {
        let Some(track) = self.project.tracks.get_mut(ti) else { return };
        let Some(wanted) = track.videos.get(choice).cloned() else { return };
        if wanted.id == track.video {
            return;
        }
        let (left, timing) = (track.video.clone(), std::mem::take(&mut track.timing));
        if let Some(video) = track.videos.iter_mut().find(|v| v.id == left) {
            video.timing = timing;
        }
        (track.video, track.timing) = (wanted.id, wanted.timing);
        self.video_corrected = Some(std::time::Instant::now());
    }

    /// What a video of the song is called in the list of a video track.
    fn video_name(number: usize, video: &VideoChoice) -> String {
        if video.label.is_empty() || video.label == "original" {
            tf!("Video {number}: the original" [number])
        } else {
            let label = &video.label;
            tf!("Video {number}: {label}" [number, label])
        }
    }

    // ----- the video player ---------------------------------------------------------

    /// The second of a recording track's video that goes with the playhead.
    fn video_second(&self, ti: usize) -> Option<f64> {
        let track = self.project.tracks.get(ti)?;
        Some(sec_at(&track.timing, self.position)? + f64::from(self.settings.video_delay_ms) / 1000.0)
    }

    /// Where the player goes when the song plays with a video that leads and
    /// no editor shows it: a corner of the tracks, for YouTube's player is
    /// only played where it is seen.
    fn video_corner(&self, ctx: &egui::Context) -> egui::Rect {
        let screen = ctx.screen_rect();
        let size = egui::vec2(320.0, 180.0);
        egui::Rect::from_min_size(screen.right_bottom() - size - egui::vec2(16.0, 40.0), size)
    }

    /// Once a frame: shows the player where an editor shows a video track, or
    /// in a corner while the song plays with a video that leads; keeps the
    /// video in step with the playhead.
    pub fn drive_video(&mut self, ctx: &egui::Context) {
        let leading = lead(&self.project).filter(|&ti| self.project.follow && self.project.tracks[ti].kind == TrackKind::Video);
        let shown = self.video_editor.filter(|&ti| self.project.tracks.get(ti).is_some_and(|t| t.kind == TrackKind::Video && !t.video.is_empty()));
        let Some(ti) = leading.or(shown) else {
            if let Some(player) = &mut self.video {
                player.place(None);
                if player.state() == PLAYING {
                    player.pause();
                }
            }
            self.video_rect = None;
            self.video_editor = None;
            return;
        };
        // In the editor that shows it; elsewhere, in a corner when the user asked for it.
        let rect = self.video_rect.take().filter(|_| shown == Some(ti)).or_else(|| self.settings.show_video.then(|| self.video_corner(ctx)));
        self.video_editor = None;
        if self.video.is_none() {
            if self.video_failed || self.window_handle == 0 {
                return;
            }
            match VideoPlayer::start(self.window_handle, &self.data_dir.join("WebView2")) {
                Ok(player) => self.video = Some(player),
                Err(e) => {
                    self.video_failed = true;
                    self.status = tf!("The video player did not start: {e}" [e]);
                    return;
                }
            }
        }
        let id = self.project.tracks[ti].video.clone();
        let wanted = self.video_second(ti).unwrap_or(0.0);
        let (volume, muted) = (self.project.tracks[ti].volume, self.project.tracks[ti].mute);
        let following = leading == Some(ti);
        let playing = self.playing && following;
        let moved = (self.position - self.video_position).abs() > 1e-3;
        // Moved by the user (the ruler, an editor), or back to the start of the
        // loop: the video goes there, rather than the song back to the video.
        let jumped = std::mem::take(&mut self.playhead_moved) || (self.playing && self.position + 0.5 < self.video_position);
        self.video_position = self.position;
        let Some(player) = &mut self.video else { return };
        player.poll();
        // The video has just started playing (and was not playing a frame ago).
        let rose = player.state() == PLAYING && player.seen_state != PLAYING;
        player.seen_state = player.state();
        if let Some(why) = player.failed.take() {
            self.video = None;
            self.video_failed = true;
            self.status = tf!("The video player stopped: {why}" [why]);
            return;
        }
        let ppp = ctx.pixels_per_point();
        player.place(rect.map(|r| [(r.left() * ppp).round() as i32, (r.top() * ppp).round() as i32, (r.width() * ppp).round() as i32, (r.height() * ppp).round() as i32]));
        if !player.api {
            ctx.request_repaint_after(Duration::from_millis(100));
            // The song waits for the player to be ready, then starts with it.
            if playing {
                self.toggle_play();
                self.video_waiting = true;
                self.status = tr("Waiting for the video…").to_owned();
            }
            return;
        }
        if player.loaded != id {
            player.load(&id, wanted);
        }
        // The track's volume, and the program's, whose usual place (0.8) is the
        // video's full volume.
        let master = f64::from(self.project.master_volume) / 0.8;
        player.set_volume(if muted { 0 } else { (f64::from(volume) * master * 100.0).round() as i32 });
        let delay = f64::from(self.settings.video_delay_ms) / 1000.0;
        // What the song does once the player is let go: move to a beat, and
        // start playing with the video.
        let (mut move_to, mut start) = (None, false);
        if playing {
            let state = player.state();
            if jumped {
                player.seek(wanted);
                self.video_corrected = Some(std::time::Instant::now());
            } else if state != PLAYING {
                if player.asked_to_play.is_none_or(|at| at.elapsed() > Duration::from_millis(800)) {
                    player.play();
                }
                // The video keeps the song waiting (an advertisement, a slow
                // connection): the song stops, and starts again with the video.
                if player.asked_to_play.is_some_and(|at| at.elapsed() > PATIENCE) {
                    self.toggle_play();
                    self.video_waiting = true;
                    self.status = tr("Waiting for the video…").to_owned();
                    return;
                }
            } else if let Some(time) = player.time() {
                // The video leads: sent elsewhere, it would stop and load again.
                // The song follows it, when they part (as the video starts, or
                // after it waited), no more than once a second.
                let settled = self.video_corrected.is_none_or(|at| at.elapsed() > FOLLOW_SETTLE);
                if (time - wanted).abs() > FOLLOW_DRIFT && settled {
                    move_to = beat_at(&self.project.tracks[ti].timing, time - delay);
                }
            }
            ctx.request_repaint_after(Duration::from_millis(50));
        } else if self.video_waiting && following {
            // The song waits for the video: it starts again where the video is.
            if jumped {
                player.seek(wanted);
            }
            if player.state() == PLAYING {
                if let Some(beat) = player.time().and_then(|t| beat_at(&self.project.tracks[ti].timing, t - delay)) {
                    self.video_waiting = false;
                    move_to = Some(beat);
                    start = true;
                }
            } else if player.state() != BUFFERING && player.asked_to_play.is_none_or(|at| at.elapsed() > Duration::from_secs(2)) {
                player.play();
            }
            ctx.request_repaint_after(Duration::from_millis(100));
        } else if self.playing {
            // The song plays as written (or with another recording): the video,
            // which is not in step then, waits where it is.
            if player.state() == PLAYING && player.asked_to_play.is_some() {
                player.pause();
            }
        } else if rose && player.asked_to_play.is_none() && player.paused.is_none_or(|at| at.elapsed() > Duration::from_millis(1500)) {
            // Started from YouTube's own player: the song starts with it.
            if let Some(beat) = player.time().and_then(|t| beat_at(&self.project.tracks[ti].timing, t - delay)) {
                move_to = Some(beat);
                start = true;
            }
        } else {
            if player.state() == PLAYING {
                player.pause();
            }
            // Stopped, the video shows the instant of the playhead when it is moved.
            if moved || jumped {
                player.seek(wanted);
            }
        }
        if let Some(beat) = move_to {
            self.video_corrected = Some(std::time::Instant::now());
            self.go_to(beat.max(0.0));
            // The program's own move: the video is already there.
            self.playhead_moved = false;
            self.video_position = self.position;
        }
        if start {
            if !following {
                self.lead_with(ti);
                self.set_follow(true);
            }
            self.toggle_play();
        }
    }

    /// The editor of a video track: the video, as large as the panel allows
    /// at its proportions, and how it is set against the score.
    pub fn video_editor_panel(&mut self, ui: &mut egui::Ui, ti: usize) {
        let Some(track) = self.project.tracks.get(ti) else { return };
        let following = self.project.follow && lead(&self.project) == Some(ti);
        let mut chosen = None;
        ui.horizontal(|ui| {
            if track.videos.len() > 1 {
                // The site gave several videos of the song: one is played, another can be.
                let current = track.videos.iter().position(|v| v.id == track.video);
                let shown = current.map_or_else(|| tf!("YouTube video {}", track.video), |i| Self::video_name(i + 1, &track.videos[i]));
                egui::ComboBox::from_id_salt(("video choice", ti))
                    .selected_text(shown)
                    .show_ui(ui, |ui| {
                        for (i, video) in track.videos.iter().enumerate() {
                            if ui.selectable_label(current == Some(i), Self::video_name(i + 1, video)).on_hover_text(tf!("YouTube video {}", video.id)).clicked() {
                                chosen = Some(i);
                            }
                        }
                    })
                    .response
                    .on_hover_text(tr("The site gave several videos of the song, each in step with the tablature: choose the one played"));
            } else {
                ui.label(tf!("YouTube video {}", track.video));
            }
            ui.separator();
            if following {
                ui.weak(tr("Leads the song: played with its original sound."));
            } else if self.project.follow {
                ui.weak(tr("Another recording leads the song."));
            } else {
                ui.weak(tr("The song is played as written: turn on “Original sound” to play it with the video."));
            }
        });
        if let Some(choice) = chosen {
            self.choose_video(ti, choice);
        }
        self.recording_shift_row(ui, ti);
        if let Some(player) = &self.video {
            if let Some(code) = player.error {
                ui.colored_label(egui::Color32::from_rgb(230, 150, 60), tf!("YouTube does not play this video here (error {code}): it may have been removed, or be blocked in your country or outside YouTube." [code]));
            }
        }
        if self.video_failed {
            ui.colored_label(egui::Color32::from_rgb(230, 150, 60), tr("The video player could not start: it needs og-video.exe and WebView2Loader.dll next to the program, and Microsoft's WebView2 runtime (part of Windows 10 and 11)."));
        }
        let room = ui.available_rect_before_wrap();
        let height = room.height().min(room.width() * 9.0 / 16.0).max(60.0);
        let rect = egui::Rect::from_min_size(room.min, egui::vec2(height * 16.0 / 9.0, height));
        ui.allocate_rect(rect, egui::Sense::hover());
        ui.painter().rect_filled(rect, 0.0, egui::Color32::BLACK);
        // Only where the editor is seen whole: elsewhere the player would cover the rest.
        if ui.clip_rect().contains_rect(rect) {
            self.video_rect = Some(rect);
        }
        self.video_editor = Some(ti);
    }

    /// The row that sets a recording against the score: earlier or later by
    /// a tenth of a second, and the delay of the video player.
    pub fn recording_shift_row(&mut self, ui: &mut egui::Ui, ti: usize) {
        ui.horizontal(|ui| {
            ui.label(tr("In step with the score:"));
            if ui.small_button(tr("◀ 0.1 s")).on_hover_text(tr("The recording comes earlier against the score")).clicked() {
                self.shift_recording(ti, 0.1);
            }
            if ui.small_button(tr("0.1 s ▶")).on_hover_text(tr("The recording comes later against the score")).clicked() {
                self.shift_recording(ti, -0.1);
            }
            if self.project.tracks.get(ti).is_some_and(|t| t.kind == TrackKind::Video) {
                ui.separator();
                ui.label(tr("Delay of the video player"));
                if ui.add(egui::DragValue::new(&mut self.settings.video_delay_ms).range(-1000..=1000).speed(5.0).suffix(" ms")).on_hover_text(tr("Video players lag a little behind; this computer's lag, for every video")).changed() {
                    self.save_settings();
                }
            }
        });
    }

    /// Whether a track is a recording of the song.
    pub fn is_recording_track(&self, ti: usize) -> bool {
        is_recording(&self.project, ti)
    }

    // ----- the music library -----------------------------------------------------------

    /// Reads the music library again, in the background.
    pub fn scan_library(&mut self) {
        let folder = self.settings.music_folder.trim();
        if folder.is_empty() {
            self.library.clear();
            return;
        }
        self.library_scan = Some(crate::library::start_scan(PathBuf::from(folder), self.library.clone()));
    }

    /// The index of the library, kept beside the settings.
    fn library_index(&self) -> Option<PathBuf> {
        self.settings_path.as_ref().and_then(|p| p.parent()).map(|dir| dir.join("library.json"))
    }

    pub fn load_library_index(&mut self) {
        if let Some(text) = self.library_index().and_then(|path| std::fs::read_to_string(path).ok()) {
            self.library = serde_json::from_str(&text).unwrap_or_default();
        }
    }

    pub fn poll_library(&mut self) {
        let Some(receiving) = &self.library_scan else { return };
        match receiving.try_recv() {
            Ok(songs) => {
                self.library = songs;
                self.library_scan = None;
                if let (Some(path), Ok(text)) = (self.library_index(), serde_json::to_string(&self.library)) {
                    let _ = std::fs::write(path, text);
                }
            }
            Err(mpsc::TryRecvError::Disconnected) => self.library_scan = None,
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    /// Chooses the folder of the music library, and reads it.
    pub fn choose_music_folder(&mut self) {
        let Some(folder) = files::choose_folder(None, None) else { return };
        self.settings.music_folder = folder.to_string_lossy().into_owned();
        self.save_settings();
        self.library.clear();
        self.scan_library();
        self.status = tf!("Your music library is read from {}.", folder.display());
    }

    /// The recording of a song in the library, if there is one.
    pub fn library_recording(&self, artist: &str, title: &str) -> Option<PathBuf> {
        crate::library::find(&self.library, artist, title).map(|song| song.path.clone())
    }
}

/// A file name, for a short label.
pub fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned())
}

impl App {
    /// The timing of the recording the song follows, when it plays with its original sound.
    pub fn followed_timing(&self) -> Option<&[Timing]> {
        let ti = lead(&self.project).filter(|_| self.project.follow)?;
        Some(&self.project.tracks[ti].timing)
    }

    /// The tempo of the recording the song follows, at the playhead.
    pub fn followed_tempo(&self) -> Option<f64> {
        let timing = self.followed_timing()?;
        let here = sec_at(timing, self.position)?;
        let further = sec_at(timing, self.position + 1.0)?;
        (further > here).then(|| 60.0 / (further - here))
    }
}
