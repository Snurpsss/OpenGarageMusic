//! The loading screen: a photograph of the author's studio, the name of the
//! program, its version, and a bar that fills as the program starts. Each
//! step of starting is done in a frame of its own, so that the screen shows
//! at once and the bar moves between the steps.

use std::time::{Duration, Instant};

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, Shape, Stroke};

use crate::app::{demo_mode, App, StartStep};

/// The picture of the loading screen, which the author supplied (see
/// "External files"); it bears the name of the program.
pub const PICTURE: &[u8] = include_bytes!("../splash/loading.jpg");
/// The loading screen is this share of the size of the picture.
pub const SCALE: f32 = 0.5;
/// The window of the program once started, and the smallest it may be made.
pub const WINDOW: egui::Vec2 = vec2(1400.0, 860.0);
pub const WINDOW_AT_LEAST: egui::Vec2 = vec2(1000.0, 620.0);
/// The screen stays at least this long, from its first frame, so that it is seen.
const AT_LEAST: Duration = Duration::from_secs(2);
/// Installed sound banks are waited for at most this long; the ones still
/// being read then join the library behind the program, as they always did.
const BANKS_AT_MOST: Duration = Duration::from_secs(20);

enum Stage {
    /// Nothing done yet: the first frame only shows the screen.
    Shown,
    /// The settings are read and the sound card is open; next comes this step.
    Step(usize),
    /// Every step is done; the sound banks are being read.
    /// `since` is when the last one came in.
    Banks { total: usize, done: usize, since: Instant },
    Running,
}

/// What is done to the program once it is ready.
type Ready = Box<dyn FnOnce(&mut App)>;

pub struct Startup {
    app: Option<App>,
    stage: Stage,
    started: Instant,
    picture: Option<egui::TextureHandle>,
    frames: u64,
    /// Whether the window has been given the size of the photograph, once
    /// the screen it is on is known.
    placed: bool,
    /// What is done to the program once it is ready (the settings of a run).
    ready: Option<Ready>,
}

impl Startup {
    pub fn new(ready: impl FnOnce(&mut App) + 'static) -> Self {
        Self { app: None, stage: Stage::Shown, started: Instant::now(), picture: None, frames: 0, placed: false, ready: Some(Box::new(ready)) }
    }

    /// How much of starting is done, from 0 to 1: the sound card and each
    /// step make the first part, the sound banks, which take longest, the rest.
    fn progress(&self) -> f32 {
        const STEPS: f32 = 0.4;
        let steps = (StartStep::ALL.len() + 1) as f32;
        match self.stage {
            Stage::Shown => 0.0,
            Stage::Step(next) => STEPS * (next + 1) as f32 / steps,
            Stage::Banks { total: 0, .. } | Stage::Running => 1.0,
            Stage::Banks { total, done, since } => {
                // How far into the bank being read is not known: the bar creeps
                // towards its end, slower and slower, without reaching it.
                let creep = 0.9 * (1.0 - (-since.elapsed().as_secs_f32() / 3.0).exp());
                STEPS + (1.0 - STEPS) * (done as f32 + creep).min(total as f32) / total as f32
            }
        }
    }

    /// Does the next thing there is to do to start.
    fn advance(&mut self) {
        let least = if demo_mode() { Duration::ZERO } else { AT_LEAST };
        if matches!(self.stage, Stage::Shown) {
            self.app = Some(App::begin());
            self.stage = Stage::Step(0);
            return;
        }
        let Some(app) = &mut self.app else { return };
        match self.stage {
            Stage::Shown | Stage::Running => {}
            Stage::Step(next) => {
                app.start_step(StartStep::ALL[next]);
                self.stage = if next + 1 < StartStep::ALL.len() {
                    Stage::Step(next + 1)
                } else {
                    Stage::Banks { total: if app.banks_loading() { app.settings.banks.len() } else { 0 }, done: 0, since: Instant::now() }
                };
            }
            Stage::Banks { total, done, since } => {
                let came = app.poll_bank_loading();
                let done = (done + came).min(total);
                self.stage = Stage::Banks { total, done, since: if came > 0 { Instant::now() } else { since } };
                let elapsed = self.started.elapsed();
                if (!app.banks_loading() || elapsed >= BANKS_AT_MOST) && elapsed >= least && !kept_for_a_picture() {
                    if let Some(ready) = self.ready.take() {
                        ready(app);
                    }
                    self.stage = Stage::Running;
                }
            }
        }
    }

    fn draw(&mut self, ctx: &egui::Context) {
        // The bar fills no faster than over the time the screen is shown at least.
        let least = if demo_mode() { 1e-3 } else { AT_LEAST.as_secs_f32() };
        let shown = if kept_for_a_picture() { 0.6 } else { self.progress().min(self.started.elapsed().as_secs_f32() / least).clamp(0.0, 1.0) };
        let picture = self.picture.get_or_insert_with(|| {
            let image = image::load_from_memory_with_format(PICTURE, image::ImageFormat::Jpeg)
                .map(|image| {
                    let rgba = image.to_rgba8();
                    egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw())
                })
                .unwrap_or_else(|_| egui::ColorImage::new([1, 1], Color32::BLACK));
            // Drawn smaller than it is: mipmaps keep it smooth.
            let options = egui::TextureOptions { mipmap_mode: Some(egui::TextureFilter::Linear), ..egui::TextureOptions::LINEAR };
            ctx.load_texture("loading screen", image, options)
        });
        let texture = picture.id();
        let size = picture.size_vec2();
        egui::CentralPanel::default().frame(egui::Frame::none().fill(Color32::BLACK)).show(ctx, |ui| {
            let screen = ui.max_rect();
            let painter = ui.painter();
            painter.image(texture, screen, cover(size, screen.size()), Color32::WHITE);
            // The bottom darkens, for the version and the bar to read on the picture.
            let fade = Rect::from_min_max(pos2(screen.left(), screen.bottom() - screen.height() * 0.3), screen.max);
            let mut mesh = egui::Mesh::default();
            mesh.colored_vertex(fade.left_top(), Color32::TRANSPARENT);
            mesh.colored_vertex(fade.right_top(), Color32::TRANSPARENT);
            mesh.colored_vertex(fade.left_bottom(), Color32::from_black_alpha(225));
            mesh.colored_vertex(fade.right_bottom(), Color32::from_black_alpha(225));
            mesh.add_triangle(0, 1, 2);
            mesh.add_triangle(1, 2, 3);
            painter.add(Shape::mesh(mesh));

            let middle = screen.center().x;
            // The name of the program is on the picture; under it, the version and the bar.
            let base = screen.bottom() - screen.height() * 0.08;
            let version = concat!("v", env!("OG_VERSION"));
            painter.text(pos2(middle, base - 20.0) + vec2(1.0, 1.0), Align2::CENTER_CENTER, version, FontId::proportional(16.0), Color32::BLACK);
            painter.text(pos2(middle, base - 20.0), Align2::CENTER_CENTER, version, FontId::proportional(16.0), Color32::WHITE);

            let track = Rect::from_center_size(pos2(middle, base), vec2((screen.width() * 0.45).clamp(240.0, 640.0), 7.0));
            painter.rect_filled(track, 4.0, Color32::from_white_alpha(40));
            let filled = Rect::from_min_size(track.min, vec2(track.width() * shown, track.height()));
            painter.rect_filled(filled, 4.0, Color32::from_rgb(70, 140, 235));
            painter.rect_stroke(track, 4.0, Stroke::new(1.0_f32, Color32::from_white_alpha(70)));
        });
    }
}

/// The loading screen stays, its bar part filled, for a picture of it in the
/// documentation (`OPENGARAGE_VIEW=loading`).
fn kept_for_a_picture() -> bool {
    std::env::var("OPENGARAGE_VIEW").is_ok_and(|view| view == "loading")
}

/// The size of the picture, in pixels.
pub fn picture_size() -> egui::Vec2 {
    image::ImageReader::new(std::io::Cursor::new(PICTURE))
        .with_guessed_format()
        .ok()
        .and_then(|reader| reader.into_dimensions().ok())
        .map_or(vec2(1600.0, 900.0), |(width, height)| vec2(width as f32, height as f32))
}

/// The size of the loading screen in points, for a picture of `pixels` on a
/// screen of `monitor` size in points: half the picture, made smaller,
/// without being stretched, when it would take more than nine tenths of the
/// screen.
pub fn screen_size(pixels: egui::Vec2, monitor: egui::Vec2) -> egui::Vec2 {
    let size = pixels * SCALE;
    let fits = (monitor.x * 0.9 / size.x).min(monitor.y * 0.9 / size.y).min(1.0);
    (size * fits).round()
}

/// Puts a window of `size` in the middle of the screen.
fn centre(ctx: &egui::Context, size: egui::Vec2) {
    if let Some(monitor) = ctx.input(|i| i.viewport().monitor_size) {
        let corner = ((monitor - size) / 2.0).max(egui::Vec2::ZERO);
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(corner.to_pos2()));
    }
}

/// The part of a picture of `picture` size that covers an area of `area`
/// size without being stretched, as texture coordinates: the middle of the
/// picture, its sides or its top and bottom cut off.
pub fn cover(picture: egui::Vec2, area: egui::Vec2) -> Rect {
    let picture_ratio = picture.x / picture.y.max(1.0);
    let area_ratio = area.x / area.y.max(1.0);
    if picture_ratio > area_ratio {
        let kept = area_ratio / picture_ratio;
        Rect::from_min_max(pos2((1.0 - kept) / 2.0, 0.0), pos2((1.0 + kept) / 2.0, 1.0))
    } else {
        let kept = picture_ratio / area_ratio;
        Rect::from_min_max(pos2(0.0, (1.0 - kept) / 2.0), pos2(1.0, (1.0 + kept) / 2.0))
    }
}

/// The window is given only its small icon, the one of its title; the task
/// bar shows the large one, which is taken from the executable, where the
/// build script put it at every size.
#[cfg(windows)]
fn taskbar_icon(frame: &eframe::Frame) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, LoadImageW, SendMessageW, SetClassLongPtrW, GCLP_HICON, GCLP_HICONSM, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_DEFAULTCOLOR, SM_CXICON, SM_CXSMICON, SM_CYICON, SM_CYSMICON, WM_SETICON,
    };
    let Ok(handle) = frame.window_handle() else { return };
    let RawWindowHandle::Win32(window) = handle.as_raw() else { return };
    let window = HWND(window.hwnd.get());
    // SAFETY: the window is the program's own and lives on; the icons are
    // read from the program's own file.
    unsafe {
        let Ok(module) = GetModuleHandleW(None) else { return };
        // The icon group the build script wrote has number 1.
        // (MAKEINTRESOURCE: the number itself stands in the place of the address.)
        let name = PCWSTR(std::ptr::without_provenance(1));
        // Given to the class of the window too: the task bar reads it there
        // without waiting for the program to answer.
        for (kind, class, width, height) in [(ICON_BIG, GCLP_HICON, SM_CXICON, SM_CYICON), (ICON_SMALL, GCLP_HICONSM, SM_CXSMICON, SM_CYSMICON)] {
            if let Ok(icon) = LoadImageW(module, name, IMAGE_ICON, GetSystemMetrics(width), GetSystemMetrics(height), LR_DEFAULTCOLOR) {
                SetClassLongPtrW(window, class, icon.0);
                SendMessageW(window, WM_SETICON, WPARAM(kind as usize), LPARAM(icon.0));
            }
        }
    }
}

impl eframe::App for Startup {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if let (Stage::Running, Some(app)) = (&self.stage, &mut self.app) {
            return app.update(ctx, frame);
        }
        // The first frame only shows the screen: opening the sound card waits
        // for the next.
        if self.frames > 0 {
            self.advance();
        } else {
            // The time the screen stays counts from when it shows, not from
            // when the window was asked for.
            self.started = Instant::now();
            #[cfg(windows)]
            taskbar_icon(frame);
        }
        self.frames += 1;
        if !self.placed {
            let monitor = ctx.input(|i| i.viewport().monitor_size);
            if let Some(monitor) = monitor.filter(|m| m.x > 1.0 && m.y > 1.0) {
                let size = screen_size(picture_size(), monitor);
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
                centre(ctx, size);
                self.placed = true;
            }
        }
        if matches!(self.stage, Stage::Running) {
            // The program draws itself from this frame on, in a window of its
            // usual size with a frame and a title.
            self.picture = None;
            ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Resizable(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(WINDOW_AT_LEAST));
            // On a screen too small for it, with room for the title and the
            // task bar, the window takes the whole screen instead.
            let fits = ctx.input(|i| i.viewport().monitor_size).is_none_or(|monitor| WINDOW.x + 40.0 <= monitor.x && WINDOW.y + 100.0 <= monitor.y);
            if fits {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(WINDOW));
                centre(ctx, WINDOW);
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
            }
            if let Some(app) = &mut self.app {
                app.update(ctx, frame);
            }
            return;
        }
        self.draw(ctx);
        ctx.request_repaint();
    }

    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        if let Some(app) = &mut self.app {
            app.on_exit(gl);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{cover, picture_size, screen_size};
    use eframe::egui::{pos2, vec2, Rect};

    #[test]
    fn the_loading_screen_is_the_size_of_the_photograph() {
        assert_eq!(picture_size(), vec2(1672.0, 941.0));
        // Half the picture on a screen that has room for it.
        assert_eq!(screen_size(vec2(1672.0, 941.0), vec2(1360.0, 768.0)), vec2(836.0, 471.0));
        // Smaller, and not stretched, on a screen too small for it.
        assert_eq!(screen_size(vec2(2000.0, 1000.0), vec2(500.0, 1000.0)), vec2(450.0, 225.0));
    }

    #[test]
    fn the_photograph_covers_the_window_without_being_stretched() {
        // Wider than the window: its sides are cut off, the middle kept.
        assert_eq!(cover(vec2(2000.0, 1000.0), vec2(1000.0, 1000.0)), Rect::from_min_max(pos2(0.25, 0.0), pos2(0.75, 1.0)));
        // Taller: its top and bottom.
        assert_eq!(cover(vec2(1000.0, 2000.0), vec2(1000.0, 1000.0)), Rect::from_min_max(pos2(0.0, 0.25), pos2(1.0, 0.75)));
        assert_eq!(cover(vec2(1600.0, 800.0), vec2(800.0, 400.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)));
    }
}
