#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use rfd::FileDialog;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;

// macOS-style semantic palette: one accent for action/selection, quiet neutrals
// for structure, and no saturated decoration competing with the task.
const APP_ACCENT: egui::Color32 = egui::Color32::from_rgb(10, 132, 255);
const APP_ACCENT_DARK: egui::Color32 = egui::Color32::from_rgb(0, 88, 188);
const APP_BACKGROUND: egui::Color32 = egui::Color32::from_rgb(242, 242, 247);
const APP_SURFACE: egui::Color32 = egui::Color32::from_rgb(255, 255, 255);
const APP_BORDER: egui::Color32 = egui::Color32::from_rgb(218, 218, 223);
const APP_MUTED: egui::Color32 = egui::Color32::from_rgb(99, 99, 102);

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum AppTab {
    #[default]
    Heic,
    Webcam,
}

#[derive(Default)]
struct Heic2JpgApp {
    active_tab: AppTab,
    inputs: Vec<PathBuf>,
    output_dir: Option<PathBuf>,
    webcam_output_dir: Option<PathBuf>,
    webcam_preview: Option<egui::TextureHandle>,
    latest_preview_jpeg: Option<Vec<u8>>,
    preview_rx: Option<Receiver<Vec<u8>>>,
    preview_stop: Option<Arc<AtomicBool>>,
    quality: u8,
    overwrite: bool,
    status: String,
    webcam_status: String,
    converting: bool,
}

impl Heic2JpgApp {
    fn new() -> Self {
        let (output_dir, webcam_output_dir, quality, overwrite, active_tab) = load_saved_settings();
        Self {
            output_dir,
            webcam_output_dir,
            quality,
            overwrite,
            active_tab,
            ..Default::default()
        }
    }

    fn choose_files(&mut self) {
        if let Some(files) = FileDialog::new()
            .add_filter("Images HEIC", &["heic", "heif"])
            .pick_files()
        {
            self.inputs = files;
            self.status = format!("{} fichier(s) sélectionné(s)", self.inputs.len());
        }
    }

    fn choose_output_dir(&mut self) {
        if let Some(dir) = FileDialog::new().pick_folder() {
            self.output_dir = Some(dir);
            self.save_settings();
        }
    }

    fn save_settings(&self) {
        save_settings(
            &self.output_dir,
            &self.webcam_output_dir,
            self.quality,
            self.overwrite,
            self.active_tab,
        );
    }

    fn convert(&mut self) {
        let Some(output_dir) = self.output_dir.clone() else {
            self.status = "Choisissez un dossier de sortie.".into();
            return;
        };
        if self.inputs.is_empty() {
            self.status = "Ajoutez au moins un fichier HEIC.".into();
            return;
        }

        self.converting = true;
        let mut converted = 0;
        let mut errors = Vec::new();

        for input in &self.inputs {
            match convert_one(input, &output_dir, self.quality, self.overwrite) {
                Ok(()) => converted += 1,
                Err(error) => errors.push(format!("{}: {error}", input.display())),
            }
        }

        self.converting = false;
        open_output_folder(&output_dir);
        self.status = if errors.is_empty() {
            format!("Conversion terminée : {converted} fichier(s).")
        } else {
            format!(
                "{converted} réussi(s), {} erreur(s). {}",
                errors.len(),
                errors.join(" | ")
            )
        };
    }
}

impl eframe::App for Heic2JpgApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(APP_BACKGROUND))
            .show(ui, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let heic_active = self.active_tab == AppTab::Heic;
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new("Convertir").strong())
                                .fill(if heic_active {
                                    APP_ACCENT
                                } else {
                                    egui::Color32::TRANSPARENT
                                })
                                .stroke(egui::Stroke::new(1.0, APP_BORDER))
                                .min_size(egui::vec2(118.0, 32.0)),
                        )
                        .clicked()
                    {
                        self.active_tab = AppTab::Heic;
                        self.save_settings();
                    }
                    let webcam_active = self.active_tab == AppTab::Webcam;
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new("Caméra").strong())
                                .fill(if webcam_active {
                                    APP_ACCENT
                                } else {
                                    egui::Color32::TRANSPARENT
                                })
                                .stroke(egui::Stroke::new(1.0, APP_BORDER))
                                .min_size(egui::vec2(102.0, 32.0)),
                        )
                        .clicked()
                    {
                        self.active_tab = AppTab::Webcam;
                        self.save_settings();
                    }
                });
                ui.add_space(8.0);
                ui.separator();
                if self.active_tab == AppTab::Heic {
                    self.stop_live_preview();
                    ui.add_space(4.0);
                    let panel_width = ui.available_width();
                    ui.allocate_ui_with_layout(
                        egui::vec2(panel_width, 82.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            egui::Frame::new()
                                .fill(APP_SURFACE)
                                .stroke(egui::Stroke::new(1.0, APP_BORDER))
                                .corner_radius(egui::CornerRadius::same(12))
                                .inner_margin(egui::Margin::symmetric(20, 14))
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        draw_camera_logo(ui);
                                        ui.vertical(|ui| {
                                            ui.heading(
                                                egui::RichText::new("Convertir des photos")
                                                    .size(22.0)
                                                    .strong(),
                                            );
                                            ui.label(
                                                egui::RichText::new(
                                                    "HEIC et HEIF vers JPEG, sur ce Mac",
                                                )
                                                .color(APP_MUTED),
                                            );
                                        });
                                    });
                                })
                        },
                    );

                    ui.add_space(14.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(panel_width, 150.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            egui::Frame::new()
                                .fill(APP_SURFACE)
                                .stroke(egui::Stroke::new(1.0, APP_BORDER))
                                .corner_radius(egui::CornerRadius::same(12))
                                .inner_margin(egui::Margin::same(18))
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.heading(
                                            egui::RichText::new("Fichiers source").size(16.0),
                                        );
                                        ui.add_space(8.0);
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "{} sélectionné(s)",
                                                self.inputs.len()
                                            ))
                                            .weak(),
                                        );
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui
                                                    .add(
                                                        egui::Button::new("Ajouter des fichiers…")
                                                            .fill(APP_ACCENT)
                                                            .stroke(egui::Stroke::NONE)
                                                            .min_size(egui::vec2(156.0, 30.0)),
                                                    )
                                                    .clicked()
                                                {
                                                    self.choose_files();
                                                }
                                            },
                                        );
                                    });
                                    ui.add_space(8.0);
                                    if self.inputs.is_empty() {
                                        ui.label(
                                            egui::RichText::new(
                                                "Ajoutez des photos HEIC ou HEIF pour commencer.",
                                            )
                                            .weak(),
                                        );
                                    } else {
                                        egui::ScrollArea::vertical().max_height(92.0).show(
                                            ui,
                                            |ui| {
                                                for input in &self.inputs {
                                                    ui.horizontal(|ui| {
                                                        ui.label(
                                                            egui::RichText::new("PHOTO")
                                                                .small()
                                                                .color(egui::Color32::from_rgb(
                                                                    55, 115, 190,
                                                                )),
                                                        );
                                                        ui.label(input.display().to_string());
                                                    });
                                                }
                                            },
                                        );
                                    }
                                })
                        },
                    );

                    ui.add_space(14.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(panel_width, 132.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            egui::Frame::new()
                                .fill(APP_SURFACE)
                                .stroke(egui::Stroke::new(1.0, APP_BORDER))
                                .corner_radius(egui::CornerRadius::same(12))
                                .inner_margin(egui::Margin::same(18))
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.heading(egui::RichText::new("Export").size(16.0));
                                    ui.add_space(8.0);
                                    ui.horizontal(|ui| {
                                        ui.label("Dossier de sortie");
                                        ui.add_space(8.0);
                                        ui.label(
                                            self.output_dir
                                                .as_ref()
                                                .map(|path| path.display().to_string())
                                                .unwrap_or_else(|| "Non sélectionné".into()),
                                        );
                                        if ui
                                            .add(egui::Button::new("Choisir…").fill(APP_ACCENT))
                                            .clicked()
                                        {
                                            self.choose_output_dir();
                                        }
                                    });
                                    if ui
                                        .add(
                                            egui::Slider::new(&mut self.quality, 1..=100)
                                                .text("Qualité JPEG"),
                                        )
                                        .changed()
                                    {
                                        self.save_settings();
                                    }
                                    if ui
                                        .checkbox(
                                            &mut self.overwrite,
                                            "Remplacer les fichiers existants",
                                        )
                                        .changed()
                                    {
                                        self.save_settings();
                                    }
                                })
                        },
                    );

                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                !self.converting,
                                egui::Button::new(egui::RichText::new("Convertir").strong())
                                    .fill(APP_ACCENT)
                                    .stroke(egui::Stroke::NONE)
                                    .min_size(egui::vec2(124.0, 34.0)),
                            )
                            .clicked()
                        {
                            self.convert();
                        }
                        if !self.status.is_empty() {
                            egui::Frame::new()
                                .fill(egui::Color32::from_rgb(232, 245, 255))
                                .stroke(egui::Stroke::new(1.0, APP_ACCENT))
                                .corner_radius(egui::CornerRadius::same(8))
                                .inner_margin(egui::Margin::symmetric(10, 6))
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new(&self.status).color(APP_ACCENT_DARK),
                                    );
                                });
                        }
                    });
                } else {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            self.ensure_live_preview();
                            self.poll_live_preview(ui.ctx());
                            self.show_webcam_tab(ui);
                        });
                }
            });
    }
}

impl Heic2JpgApp {
    fn show_webcam_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(14.0);
        egui::Frame::new()
            .fill(APP_SURFACE)
            .stroke(egui::Stroke::new(1.0, APP_BORDER))
            .corner_radius(egui::CornerRadius::same(12))
            .inner_margin(egui::Margin::symmetric(20, 18))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.heading(egui::RichText::new("Prendre une photo").size(22.0).strong());
                ui.label(
                    egui::RichText::new("Capture JPEG depuis la caméra de votre Mac.")
                        .color(APP_MUTED),
                );
                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    ui.label("Dossier de sortie");
                    ui.label(
                        self.webcam_output_dir
                            .as_ref()
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|| "Non sélectionné".into()),
                    );
                    if ui
                        .add(egui::Button::new("Choisir…").fill(APP_ACCENT))
                        .clicked()
                    {
                        if let Some(dir) = FileDialog::new().pick_folder() {
                            self.webcam_output_dir = Some(dir);
                            self.save_settings();
                        }
                    }
                });
                ui.add_space(18.0);
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new("Prendre une photo").strong())
                            .fill(APP_ACCENT)
                            .stroke(egui::Stroke::NONE)
                            .min_size(egui::vec2(164.0, 36.0)),
                    )
                    .clicked()
                {
                    self.capture_webcam(ui.ctx());
                }
                ui.add_space(14.0);
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(248, 250, 251))
                    .stroke(egui::Stroke::new(1.0, APP_BORDER))
                    .corner_radius(egui::CornerRadius::same(12))
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label(egui::RichText::new("Aperçu caméra").strong());
                        ui.add_space(6.0);
                        if let Some(texture) = &self.webcam_preview {
                            // Garde l'image entière dans la fenêtre, même si celle-ci est réduite.
                            let max_width = ui.available_width().max(240.0);
                            let max_height = (ui.available_height() - 18.0).max(180.0);
                            let height_from_width = max_width * 9.0 / 16.0;
                            let preview_height = height_from_width.min(max_height);
                            let preview_width = preview_height * 16.0 / 9.0;
                            ui.horizontal(|ui| {
                                ui.add_space(
                                    ((ui.available_width() - preview_width).max(0.0)) / 2.0,
                                );
                                ui.add(
                                    egui::Image::new(texture).fit_to_exact_size(egui::vec2(
                                        preview_width,
                                        preview_height,
                                    )),
                                );
                            });
                        } else {
                            ui.centered_and_justified(|ui| {
                                ui.label(egui::RichText::new("Connexion à la caméra…").weak());
                            });
                        }
                    });
                if !self.webcam_status.is_empty() {
                    ui.add_space(12.0);
                    egui::Frame::new()
                        .fill(egui::Color32::from_rgb(232, 245, 255))
                        .stroke(egui::Stroke::new(1.0, APP_ACCENT))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(egui::Margin::symmetric(10, 6))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(&self.webcam_status).color(APP_ACCENT_DARK),
                            );
                        });
                }
            });
    }

    fn capture_webcam(&mut self, ctx: &egui::Context) {
        let Some(output_dir) = self.webcam_output_dir.clone() else {
            self.webcam_status = "Choisissez un dossier de sortie.".into();
            return;
        };
        let filename = format!("webcam-{}.jpg", chrono_like_timestamp());
        let output = output_dir.join(filename);
        let Some(bytes) = self.latest_preview_jpeg.clone() else {
            self.webcam_status = "Aperçu caméra indisponible.".into();
            return;
        };
        self.webcam_status = match std::fs::write(&output, &bytes) {
            Ok(()) => {
                self.load_preview_from_bytes(ctx, &bytes);
                open_output_folder(&output_dir);
                format!("Photo enregistrée : {}", output.display())
            }
            Err(error) => format!("Capture impossible : {error}"),
        };
    }

    fn load_preview_from_bytes(&mut self, ctx: &egui::Context, bytes: &[u8]) {
        if let Ok(image) = image::load_from_memory(bytes) {
            let image = image.to_rgba8();
            let size = [image.width() as usize, image.height() as usize];
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
            self.webcam_preview =
                Some(ctx.load_texture("webcam-preview", color_image, egui::TextureOptions::LINEAR));
        }
    }

    fn ensure_live_preview(&mut self) {
        if self.preview_rx.is_some() {
            return;
        }
        request_camera_permission();
        let Some(stream_bin) = camera_stream_path() else {
            self.webcam_status = "Helper caméra absent. Rebuild the .app.".into();
            return;
        };
        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = Arc::clone(&stop);
        thread::spawn(move || {
            let Ok(mut child) = Command::new(stream_bin)
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
            else {
                return;
            };
            let Some(mut stdout) = child.stdout.take() else {
                return;
            };
            let mut length = [0_u8; 4];
            while !stop_thread.load(Ordering::Relaxed) {
                if stdout.read_exact(&mut length).is_err() {
                    break;
                }
                let size = u32::from_be_bytes(length) as usize;
                if size == 0 || size > 20_000_000 {
                    break;
                }
                let mut frame = vec![0_u8; size];
                if stdout.read_exact(&mut frame).is_err() {
                    break;
                }
                if tx.send(frame).is_err() {
                    break;
                }
            }
            let _ = child.kill();
        });
        self.preview_stop = Some(stop);
        self.preview_rx = Some(rx);
    }

    fn stop_live_preview(&mut self) {
        if let Some(stop) = self.preview_stop.take() {
            stop.store(true, Ordering::Relaxed);
        }
        self.preview_rx = None;
    }

    fn poll_live_preview(&mut self, ctx: &egui::Context) {
        let latest = self.preview_rx.as_ref().and_then(|rx| rx.try_iter().last());
        if let Some(bytes) = latest {
            self.latest_preview_jpeg = Some(bytes.clone());
            if let Ok(image) = image::load_from_memory(&bytes) {
                let image = image.to_rgba8();
                let size = [image.width() as usize, image.height() as usize];
                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
                self.webcam_preview = Some(ctx.load_texture(
                    "webcam-live-preview",
                    color_image,
                    egui::TextureOptions::LINEAR,
                ));
                self.webcam_status.clear();
            }
        }
        if self.preview_rx.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }
}

fn chrono_like_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "capture".into())
}

fn request_camera_permission() {
    let helper = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join("camera_permission")))
        .filter(|path| path.exists())
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/camera_permission")
        });
    if helper.exists() {
        let _ = Command::new(helper).status();
    }
}

fn camera_stream_path() -> Option<PathBuf> {
    let packaged = std::env::current_exe()
        .ok()?
        .parent()?
        .join("camera_stream");
    if packaged.exists() {
        return Some(packaged);
    }
    let development = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/camera_stream");
    development.exists().then_some(development)
}

fn default_downloads() -> PathBuf {
    dirs::download_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join("Downloads")))
        .unwrap_or_else(|| PathBuf::from("Downloads"))
}

fn open_output_folder(path: &Path) {
    let _ = Command::new("open").arg(path).spawn();
}

fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("HEIC2JPG").join("directories.conf"))
}

fn load_saved_settings() -> (Option<PathBuf>, Option<PathBuf>, u8, bool, AppTab) {
    let defaults = default_downloads();
    let Some(path) = config_path() else {
        return (
            Some(defaults.clone()),
            Some(defaults),
            90,
            false,
            AppTab::Heic,
        );
    };
    let Ok(contents) = std::fs::read_to_string(path) else {
        return (
            Some(defaults.clone()),
            Some(defaults),
            90,
            false,
            AppTab::Heic,
        );
    };
    let mut heic = None;
    let mut webcam = None;
    let mut quality = 90;
    let mut overwrite = false;
    let mut active_tab = AppTab::Heic;
    for line in contents.lines() {
        if let Some(value) = line.strip_prefix("heic=") {
            heic = Some(PathBuf::from(value));
        }
        if let Some(value) = line.strip_prefix("webcam=") {
            webcam = Some(PathBuf::from(value));
        }
        if let Some(value) = line.strip_prefix("quality=") {
            quality = value.parse().unwrap_or(90).clamp(1, 100);
        }
        if let Some(value) = line.strip_prefix("overwrite=") {
            overwrite = value == "true";
        }
        if let Some(value) = line.strip_prefix("tab=") {
            active_tab = (value == "webcam")
                .then_some(AppTab::Webcam)
                .unwrap_or(AppTab::Heic);
        }
    }
    (
        Some(heic.unwrap_or_else(|| defaults.clone())),
        Some(webcam.unwrap_or(defaults)),
        quality,
        overwrite,
        active_tab,
    )
}

fn save_settings(
    heic: &Option<PathBuf>,
    webcam: &Option<PathBuf>,
    quality: u8,
    overwrite: bool,
    active_tab: AppTab,
) {
    let Some(path) = config_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let content = format!(
        "heic={}\nwebcam={}\nquality={}\noverwrite={}\ntab={}\n",
        heic.as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        webcam
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        quality,
        overwrite,
        if active_tab == AppTab::Webcam {
            "webcam"
        } else {
            "heic"
        }
    );
    let _ = std::fs::write(path, content);
}

fn draw_camera_logo(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(64.0, 48.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let body = rect.shrink2(egui::vec2(2.0, 5.0));
    let accent = egui::Color32::from_rgb(40, 96, 160);
    let lens = egui::Color32::from_rgb(226, 242, 255);

    painter.rect_filled(body, 9.0, accent);
    painter.rect_stroke(
        body,
        9.0,
        egui::Stroke::new(1.5, egui::Color32::from_rgb(24, 64, 110)),
        egui::StrokeKind::Outside,
    );
    painter.rect_filled(
        egui::Rect::from_min_size(
            egui::pos2(body.left() + 9.0, body.top() - 4.0),
            egui::vec2(18.0, 8.0),
        ),
        3.0,
        accent,
    );
    painter.circle_filled(body.center(), 13.0, egui::Color32::from_rgb(24, 64, 110));
    painter.circle_filled(body.center(), 9.0, lens);
    painter.circle_filled(
        body.center() - egui::vec2(3.0, 3.0),
        3.0,
        egui::Color32::WHITE,
    );
    painter.circle_filled(
        egui::pos2(body.right() - 10.0, body.top() + 10.0),
        2.5,
        egui::Color32::from_rgb(230, 245, 255),
    );
}

fn convert_one(
    input: &Path,
    output_dir: &Path,
    quality: u8,
    overwrite: bool,
) -> Result<(), String> {
    let stem = input
        .file_stem()
        .ok_or_else(|| "nom de fichier invalide".to_string())?;
    let output = output_dir.join(format!("{}.jpg", stem.to_string_lossy()));

    if output.exists() && !overwrite {
        return Err(format!("sortie existante : {}", output.display()));
    }

    let result = Command::new("/usr/bin/sips")
        .arg("-s")
        .arg("format")
        .arg("jpeg")
        .arg("-s")
        .arg("formatOptions")
        .arg(quality.to_string())
        .arg(input)
        .arg("--out")
        .arg(&output)
        .output()
        .map_err(|error| format!("impossible de lancer sips: {error}"))?;

    if result.status.success() {
        Ok(())
    } else {
        let details = String::from_utf8_lossy(&result.stderr);
        Err(format!("sips a échoué ({})", details.trim()))
    }
}

fn main() -> eframe::Result<()> {
    let app_icon =
        eframe::icon_data::from_png_bytes(include_bytes!("../assets/app_icon_runtime.png"))
            .expect("icône application invalide");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([760.0, 620.0])
            .with_min_inner_size([640.0, 520.0])
            .with_icon(app_icon),
        ..Default::default()
    };

    eframe::run_native(
        "HEIC → JPG",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(mac_visuals());
            Ok(Box::new(Heic2JpgApp::new()))
        }),
    )
}

fn mac_visuals() -> egui::Visuals {
    let mut visuals = egui::Visuals::light();
    visuals.panel_fill = APP_BACKGROUND;
    visuals.window_fill = APP_BACKGROUND;
    visuals.faint_bg_color = egui::Color32::from_rgb(232, 232, 237);
    visuals.extreme_bg_color = egui::Color32::WHITE;
    visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(245, 245, 250);
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(238, 238, 243);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(220, 235, 251);
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(194, 220, 249);
    visuals.selection.bg_fill = APP_ACCENT;
    visuals.hyperlink_color = APP_ACCENT_DARK;
    visuals
}
