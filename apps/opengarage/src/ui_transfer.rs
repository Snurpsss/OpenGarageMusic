//! The windows of File → "Export to…" and "Import into this project…": the
//! formats the song is exported as, and what can be added to the project.

use eframe::egui;

use crate::app::App;
use crate::export::Compressed;
use crate::i18n::tr;
use crate::widgets::radio_list;

/// What the song is exported as.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExportFormat {
    #[default]
    Wav16,
    Wav24,
    Aiff16,
    Aiff24,
    Mp3,
    Aac,
    Midi,
}

impl ExportFormat {
    pub const ALL: [ExportFormat; 7] = [ExportFormat::Wav16, ExportFormat::Wav24, ExportFormat::Aiff16, ExportFormat::Aiff24, ExportFormat::Mp3, ExportFormat::Aac, ExportFormat::Midi];

    pub fn label(self) -> String {
        match self {
            ExportFormat::Wav16 => tf!("WAV, {} bits", 16),
            ExportFormat::Wav24 => tf!("WAV, {} bits", 24),
            ExportFormat::Aiff16 => tf!("AIFF, {} bits", 16),
            ExportFormat::Aiff24 => tf!("AIFF, {} bits", 24),
            ExportFormat::Mp3 => "MP3".to_owned(),
            ExportFormat::Aac => "AAC (.m4a)".to_owned(),
            ExportFormat::Midi => "MIDI (.mid)".to_owned(),
        }
    }

    /// What the format is good for, in a few words.
    pub fn purpose(self) -> &'static str {
        match self {
            ExportFormat::Wav16 => tr("Lossless, as on a CD: for any program or player"),
            ExportFormat::Wav24 => tr("Lossless and finer: to go on mixing or mastering elsewhere"),
            ExportFormat::Aiff16 | ExportFormat::Aiff24 => tr("Lossless, the usual format on a Mac"),
            ExportFormat::Mp3 => tr("Small, to share or to listen to on a phone"),
            ExportFormat::Aac => tr("Small, and a little better than MP3 at the same size"),
            ExportFormat::Midi => tr("The notes of the tracks, without sound: for another program or a score editor"),
        }
    }
}

impl App {
    /// The formats, the quality of MP3 and AAC, and the button that asks where.
    pub fn export_window(&mut self, ui: &mut egui::Ui) -> bool {
        ui.label(tr("Export the song as:"));
        ui.add_space(4.0);
        for format in ExportFormat::ALL {
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.export_format, format, format.label());
                ui.weak(format.purpose());
            });
        }
        if matches!(self.export_format, ExportFormat::Mp3 | ExportFormat::Aac) {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(tr("Quality of MP3 and AAC"));
                let current = self.export_kbps();
                let rates = [(128u32, tr("smaller file")), (192, tr("good")), (256, tr("very good")), (320, tr("best, MP3 only"))];
                if let Some(kbps) = radio_list(ui, &current, rates.map(|(kbps, what)| (kbps, format!("{kbps} kb/s  ·  {what}")))) {
                    self.settings.export_kbps = kbps;
                    self.save_settings();
                }
            });
        }
        ui.add_space(6.0);
        ui.weak(tr("The whole song is exported, from its start to its end marker."));
        ui.add_space(6.0);
        if !ui.button(tr("Export…")).clicked() {
            return false;
        }
        match self.export_format {
            ExportFormat::Wav16 | ExportFormat::Wav24 => {
                self.export_bits = if self.export_format == ExportFormat::Wav24 { 24 } else { 16 };
                self.export_wav();
            }
            ExportFormat::Aiff16 | ExportFormat::Aiff24 => {
                self.export_bits = if self.export_format == ExportFormat::Aiff24 { 24 } else { 16 };
                self.export_aiff();
            }
            ExportFormat::Mp3 => self.export_compressed(Compressed::Mp3),
            ExportFormat::Aac => self.export_compressed(Compressed::Aac),
            ExportFormat::Midi => self.export_midi(),
        }
        true
    }

    /// What can be added to the project open now.
    pub fn import_window(&mut self, ui: &mut egui::Ui) -> bool {
        ui.label(tr("Add to this project:"));
        ui.add_space(4.0);
        let mut done = false;
        ui.horizontal(|ui| {
            if ui.button(tr("An audio file (WAV, MP3, AAC)…")).clicked() {
                self.import_audio();
                done = true;
            }
            ui.weak(tr("On the selected audio track, or a new one, at the playhead"));
        });
        ui.horizontal(|ui| {
            if ui.button(tr("A MIDI file…")).clicked() {
                self.import_midi();
                done = true;
            }
            ui.weak(tr("Its tracks of notes, as new tracks"));
        });
        ui.horizontal(|ui| {
            if ui.button(tr("A recording of the song…")).clicked() {
                self.link_recording_dialog();
                done = true;
            }
            ui.weak(tr("MP3, OGG, FLAC, M4A or WAV: found in step with the score, to play the song with its original sound"));
        });
        ui.add_space(6.0);
        ui.weak(tr("A file can also be dropped onto the window. To make a new project of a Guitar Pro or MusicXML file, or of a tablature site, use the File menu: they replace the project."));
        done
    }
}
