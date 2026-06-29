use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use eframe::egui::{self, Color32, CornerRadius, Frame, Margin, RichText, Stroke, Vec2};

use crate::download::{self, DownloadProgress};
use crate::torrent::TorrentMetadata;
use crate::tracker::generate_peer_id;
use crate::{discover_peers, format_bytes, hex_encode, DiscoverConfig};

#[derive(Clone, Copy)]
struct AppTheme {
    bg: Color32,
    surface: Color32,
    surface_alt: Color32,
    border: Color32,
    text: Color32,
    text_dim: Color32,
    accent: Color32,
    accent_dim: Color32,
    accent_text: Color32,
    success: Color32,
    warning: Color32,
    error: Color32,
    hover: Color32,
    extreme_bg: Color32,
}

impl AppTheme {
    fn dark() -> Self {
        Self {
            bg: Color32::from_rgb(18, 20, 26),
            surface: Color32::from_rgb(28, 31, 40),
            surface_alt: Color32::from_rgb(36, 40, 52),
            border: Color32::from_rgb(52, 58, 72),
            text: Color32::from_rgb(232, 236, 244),
            text_dim: Color32::from_rgb(148, 156, 172),
            accent: Color32::from_rgb(56, 198, 186),
            accent_dim: Color32::from_rgb(38, 120, 112),
            accent_text: Color32::from_rgb(18, 20, 26),
            success: Color32::from_rgb(88, 196, 130),
            warning: Color32::from_rgb(232, 180, 88),
            error: Color32::from_rgb(232, 108, 108),
            hover: Color32::from_rgb(44, 49, 62),
            extreme_bg: Color32::from_rgb(12, 14, 18),
        }
    }

    fn light() -> Self {
        Self {
            bg: Color32::from_rgb(244, 246, 250),
            surface: Color32::from_rgb(255, 255, 255),
            surface_alt: Color32::from_rgb(236, 240, 246),
            border: Color32::from_rgb(208, 214, 226),
            text: Color32::from_rgb(28, 32, 42),
            text_dim: Color32::from_rgb(96, 104, 120),
            accent: Color32::from_rgb(0, 150, 136),
            accent_dim: Color32::from_rgb(0, 122, 110),
            accent_text: Color32::from_rgb(255, 255, 255),
            success: Color32::from_rgb(34, 150, 84),
            warning: Color32::from_rgb(196, 130, 24),
            error: Color32::from_rgb(196, 64, 64),
            hover: Color32::from_rgb(224, 230, 240),
            extreme_bg: Color32::from_rgb(228, 232, 240),
        }
    }
}

enum WorkerMessage {
    Log(String),
    Peers(Vec<SocketAddr>),
    Progress(DownloadProgress),
    Finished(PathBuf),
    Failed(String),
}

struct BackgroundWorker {
    receiver: Receiver<WorkerMessage>,
}

pub struct TorrentApp {
    peer_id: [u8; 20],
    torrent_path: Option<PathBuf>,
    metadata: Option<TorrentMetadata>,
    output_dir: PathBuf,
    peers: Vec<SocketAddr>,
    worker: Option<BackgroundWorker>,
    progress: DownloadProgress,
    status: String,
    logs: Vec<String>,
    use_dht: bool,
    download_complete: bool,
    dark_mode: bool,
    theme: AppTheme,
}

impl Default for TorrentApp {
    fn default() -> Self {
        Self {
            peer_id: generate_peer_id(),
            torrent_path: None,
            metadata: None,
            output_dir: PathBuf::from("downloads"),
            peers: Vec::new(),
            worker: None,
            progress: DownloadProgress {
                current_piece: 0,
                total_pieces: 0,
                downloaded_bytes: 0,
                total_bytes: 0,
            },
            status: "Open a .torrent file to begin.".into(),
            logs: Vec::new(),
            use_dht: true,
            download_complete: false,
            dark_mode: true,
            theme: AppTheme::dark(),
        }
    }
}

fn apply_theme(ctx: &egui::Context, theme: &AppTheme, dark_mode: bool) {
    let mut visuals = if dark_mode {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };

    visuals.panel_fill = theme.bg;
    visuals.window_fill = theme.bg;
    visuals.extreme_bg_color = theme.extreme_bg;
    visuals.faint_bg_color = theme.surface_alt;
    visuals.widgets.noninteractive.bg_fill = theme.surface_alt;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, theme.text_dim);
    visuals.widgets.inactive.bg_fill = theme.surface;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, theme.text_dim);
    visuals.widgets.hovered.bg_fill = theme.hover;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, theme.text);
    visuals.widgets.active.bg_fill = theme.accent_dim;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, theme.text);
    visuals.widgets.open.bg_fill = theme.accent_dim;
    visuals.selection.bg_fill = theme.accent.gamma_multiply(0.25);
    visuals.selection.stroke = Stroke::new(1.0, theme.accent);
    visuals.hyperlink_color = theme.accent;
    visuals.warn_fg_color = theme.warning;
    visuals.error_fg_color = theme.error;
    visuals.window_corner_radius = CornerRadius::same(10);
    visuals.menu_corner_radius = CornerRadius::same(8);
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(10.0, 8.0);
    style.spacing.button_padding = Vec2::new(16.0, 9.0);
    style.spacing.indent = 18.0;
    ctx.set_style(style);
}

fn card_frame(theme: &AppTheme) -> Frame {
    Frame::new()
        .fill(theme.surface)
        .stroke(Stroke::new(1.0, theme.border))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(16))
}

fn accent_button(ui: &mut egui::Ui, theme: &AppTheme, label: &str, enabled: bool) -> egui::Response {
    let text = RichText::new(label)
        .size(14.0)
        .color(if enabled { theme.accent_text } else { theme.text_dim });
    let fill = if enabled { theme.accent } else { theme.surface_alt };
    ui.add_enabled(
        enabled,
        egui::Button::new(text)
            .fill(fill)
            .stroke(Stroke::new(
                1.0,
                if enabled { theme.accent } else { theme.border },
            ))
            .corner_radius(CornerRadius::same(8)),
    )
}

fn secondary_button(ui: &mut egui::Ui, theme: &AppTheme, label: &str, enabled: bool) -> egui::Response {
    let text = RichText::new(label)
        .size(14.0)
        .color(if enabled { theme.text } else { theme.text_dim });
    ui.add_enabled(
        enabled,
        egui::Button::new(text)
            .fill(theme.surface_alt)
            .stroke(Stroke::new(1.0, theme.border))
            .corner_radius(CornerRadius::same(8)),
    )
}

fn stat_cell(ui: &mut egui::Ui, theme: &AppTheme, label: &str, value: impl Into<RichText>) {
    ui.vertical(|ui| {
        ui.label(
            RichText::new(label)
                .size(11.0)
                .color(theme.text_dim)
                .strong(),
        );
        ui.add_space(2.0);
        ui.label(value.into().size(14.0).color(theme.text));
    });
}

impl TorrentApp {
    fn busy(&self) -> bool {
        self.worker.is_some()
    }

    fn status_color(&self) -> Color32 {
        if self.download_complete {
            self.theme.success
        } else if self.busy() {
            self.theme.accent
        } else if self.status.starts_with("Error") || self.status.starts_with("Failed") {
            self.theme.error
        } else if self.peers.is_empty() && self.metadata.is_some() {
            self.theme.warning
        } else {
            self.theme.text_dim
        }
    }

    fn set_dark_mode(&mut self, ctx: &egui::Context, dark_mode: bool) {
        self.dark_mode = dark_mode;
        self.theme = if dark_mode {
            AppTheme::dark()
        } else {
            AppTheme::light()
        };
        apply_theme(ctx, &self.theme, self.dark_mode);
    }

    fn push_log(&mut self, line: impl Into<String>) {
        self.logs.push(line.into());
        if self.logs.len() > 200 {
            self.logs.remove(0);
        }
    }

    fn poll_worker(&mut self, ctx: &egui::Context) {
        loop {
            let message = {
                let Some(worker) = &self.worker else {
                    return;
                };
                match worker.receiver.try_recv() {
                    Ok(message) => message,
                    Err(TryRecvError::Empty) => return,
                    Err(TryRecvError::Disconnected) => {
                        self.status = "Background task ended unexpectedly.".into();
                        self.worker = None;
                        return;
                    }
                }
            };

            match message {
                WorkerMessage::Log(line) => self.push_log(line),
                WorkerMessage::Peers(peers) => {
                    self.peers = peers;
                    self.status = format!("Found {} peers", self.peers.len());
                    self.push_log(self.status.clone());
                }
                WorkerMessage::Progress(progress) => {
                    self.progress = progress;
                    self.status = format!(
                        "Downloading piece {}/{}",
                        self.progress.current_piece, self.progress.total_pieces
                    );
                }
                WorkerMessage::Finished(path) => {
                    self.download_complete = true;
                    self.status = format!("Download complete: {}", path.display());
                    self.push_log(self.status.clone());
                    self.worker = None;
                    ctx.request_repaint();
                    return;
                }
                WorkerMessage::Failed(message) => {
                    self.status = message.clone();
                    self.push_log(format!("Error: {message}"));
                    self.worker = None;
                    ctx.request_repaint();
                    return;
                }
            }
        }
    }

    fn open_torrent_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Torrent", &["torrent"])
            .pick_file()
        else {
            return;
        };

        match TorrentMetadata::from_path(&path) {
            Ok(metadata) => {
                self.output_dir = PathBuf::from(&metadata.name);
                self.torrent_path = Some(path.clone());
                self.metadata = Some(metadata);
                self.peers.clear();
                self.download_complete = false;
                self.progress = DownloadProgress {
                    current_piece: 0,
                    total_pieces: self.metadata.as_ref().map(|m| m.piece_count()).unwrap_or(0),
                    downloaded_bytes: 0,
                    total_bytes: self.metadata.as_ref().map(|m| m.total_size()).unwrap_or(0),
                };
                self.status = format!("Loaded {}", path.display());
                self.push_log(self.status.clone());
            }
            Err(err) => {
                self.status = format!("Failed to load torrent: {err:#}");
                self.push_log(self.status.clone());
            }
        }
    }

    fn pick_output_dir(&mut self) {
        let Some(path) = rfd::FileDialog::new().pick_folder() else {
            return;
        };
        self.output_dir = path;
        self.push_log(format!("Output directory: {}", self.output_dir.display()));
    }

    fn start_find_peers(&mut self, ctx: &egui::Context) {
        let Some(metadata) = self.metadata.clone() else {
            return;
        };

        let (tx, rx) = mpsc::channel();
        let peer_id = self.peer_id;
        let use_dht = self.use_dht;
        let ctx = ctx.clone();

        self.status = "Finding peers...".into();
        self.push_log("Finding peers...".to_string());
        self.worker = Some(BackgroundWorker { receiver: rx });

        std::thread::spawn(move || {
            let _ = tx.send(WorkerMessage::Log(
                "Querying tracker and DHT...".to_string(),
            ));

            let result = discover_peers(
                &metadata,
                peer_id,
                &DiscoverConfig {
                    use_tracker: metadata.announce.is_some(),
                    use_dht,
                    port: 6881,
                },
            );

            match result {
                Ok(peers) => {
                    let _ = tx.send(WorkerMessage::Peers(peers));
                }
                Err(err) => {
                    let _ = tx.send(WorkerMessage::Failed(format!("{err:#}")));
                }
            }

            ctx.request_repaint();
        });
    }

    fn start_download(&mut self, ctx: &egui::Context) {
        let Some(metadata) = self.metadata.clone() else {
            return;
        };

        if self.peers.is_empty() {
            self.status = "Find peers before downloading.".into();
            return;
        }

        let (tx, rx) = mpsc::channel();
        let peer_id = self.peer_id;
        let peers = self.peers.clone();
        let output_dir = self.output_dir.clone();
        let ctx = ctx.clone();

        self.download_complete = false;
        self.progress = DownloadProgress {
            current_piece: 0,
            total_pieces: metadata.piece_count(),
            downloaded_bytes: 0,
            total_bytes: metadata.total_size(),
        };
        self.status = "Starting download...".into();
        self.push_log(format!(
            "Downloading to {}...",
            output_dir.display()
        ));
        self.worker = Some(BackgroundWorker { receiver: rx });

        std::thread::spawn(move || {
            let (progress_tx, progress_rx) = mpsc::channel();
            let progress_forward = tx.clone();
            let progress_ctx = ctx.clone();

            std::thread::spawn(move || {
                while let Ok(update) = progress_rx.recv() {
                    let _ = progress_forward.send(WorkerMessage::Progress(update));
                    progress_ctx.request_repaint();
                }
            });

            match download::download_torrent(
                &metadata,
                &peers,
                &output_dir,
                peer_id,
                Some(progress_tx),
            ) {
                Ok(()) => {
                    let _ = tx.send(WorkerMessage::Finished(output_dir));
                }
                Err(err) => {
                    let _ = tx.send(WorkerMessage::Failed(format!("{err:#}")));
                }
            }

            ctx.request_repaint();
        });
    }

    fn show_header(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let theme = self.theme;
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("BitTorrent Client")
                        .size(26.0)
                        .color(theme.text)
                        .strong(),
                );
                ui.label(
                    RichText::new("Open a torrent, find peers, and download.")
                        .size(13.0)
                        .color(theme.text_dim),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new("Rust").size(11.0).color(theme.accent).monospace());
                ui.add_space(12.0);

                let toggle_label = if self.dark_mode {
                    "Light mode"
                } else {
                    "Dark mode"
                };
                if secondary_button(ui, &theme, toggle_label, true).clicked() {
                    self.set_dark_mode(ctx, !self.dark_mode);
                }
            });
        });
    }

    fn show_toolbar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let theme = self.theme;
        let busy = self.busy();
        let has_torrent = self.metadata.is_some();
        let can_download = has_torrent && !self.peers.is_empty();

        card_frame(&theme).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if accent_button(ui, &theme, "Open .torrent", !busy).clicked() {
                    self.open_torrent_dialog();
                }
                if secondary_button(ui, &theme, "Find peers", !busy && has_torrent).clicked() {
                    self.start_find_peers(ctx);
                }
                if accent_button(ui, &theme, "Download", !busy && can_download).clicked() {
                    self.start_download(ctx);
                }
                if secondary_button(ui, &theme, "Output folder", !busy && has_torrent).clicked() {
                    self.pick_output_dir();
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                ui.checkbox(
                    &mut self.use_dht,
                    RichText::new("Use DHT").color(theme.text),
                );
            });
        });
    }

    fn show_status(&self, ui: &mut egui::Ui) {
        let theme = self.theme;
        let color = self.status_color();
        card_frame(&theme).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("●").size(14.0).color(color));
                ui.label(
                    RichText::new("Status")
                        .size(12.0)
                        .color(theme.text_dim)
                        .strong(),
                );
                ui.label(RichText::new(&self.status).size(14.0).color(theme.text));
            });
        });
    }

    fn show_empty_state(&self, ui: &mut egui::Ui) {
        let theme = self.theme;
        card_frame(&theme).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(28.0);
                ui.label(RichText::new("⬡").size(42.0).color(theme.accent_dim));
                ui.add_space(8.0);
                ui.label(
                    RichText::new("No torrent loaded")
                        .size(18.0)
                        .color(theme.text)
                        .strong(),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Click Open .torrent to get started.")
                        .size(13.0)
                        .color(theme.text_dim),
                );
                ui.add_space(28.0);
            });
        });
    }

    fn show_torrent_details(&self, ui: &mut egui::Ui) {
        let Some(metadata) = &self.metadata else {
            return;
        };
        let theme = self.theme;

        card_frame(&theme).show(ui, |ui| {
            ui.label(
                RichText::new(&metadata.name)
                    .size(20.0)
                    .color(theme.text)
                    .strong(),
            );
            ui.add_space(12.0);

            ui.columns(3, |columns| {
                stat_cell(
                    &mut columns[0],
                    &theme,
                    "SIZE",
                    format_bytes(metadata.total_size()),
                );
                stat_cell(
                    &mut columns[1],
                    &theme,
                    "PIECES",
                    metadata.piece_count().to_string(),
                );
                stat_cell(
                    &mut columns[2],
                    &theme,
                    "PEERS",
                    self.peers.len().to_string(),
                );
            });

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);

            stat_cell(
                ui,
                &theme,
                "INFO HASH",
                RichText::new(hex_encode(metadata.info_hash))
                    .monospace()
                    .color(theme.accent),
            );
            ui.add_space(8.0);
            stat_cell(
                ui,
                &theme,
                "OUTPUT",
                RichText::new(self.output_dir.display().to_string()).monospace(),
            );

            if let Some(path) = &self.torrent_path {
                ui.add_space(8.0);
                stat_cell(
                    ui,
                    &theme,
                    "TORRENT FILE",
                    RichText::new(path.display().to_string())
                        .monospace()
                        .color(theme.text_dim),
                );
            }

            if self.progress.total_bytes > 0
                && (self.busy() || self.download_complete || self.progress.downloaded_bytes > 0)
            {
                let fraction =
                    self.progress.downloaded_bytes as f32 / self.progress.total_bytes as f32;
                ui.add_space(14.0);
                ui.label(
                    RichText::new("DOWNLOAD PROGRESS")
                        .size(11.0)
                        .color(theme.text_dim)
                        .strong(),
                );
                ui.add_space(6.0);
                ui.add(
                    egui::ProgressBar::new(fraction)
                        .fill(theme.accent)
                        .corner_radius(CornerRadius::same(6))
                        .animate(self.busy())
                        .text(format!(
                            "{} / {}  ·  piece {}/{}",
                            format_bytes(self.progress.downloaded_bytes),
                            format_bytes(self.progress.total_bytes),
                            self.progress.current_piece,
                            self.progress.total_pieces
                        )),
                );
            }

            ui.add_space(10.0);
            egui::CollapsingHeader::new(
                RichText::new("Files")
                    .size(14.0)
                    .color(theme.text)
                    .strong(),
            )
            .default_open(true)
            .show(ui, |ui| {
                Frame::new()
                    .fill(theme.surface_alt)
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::same(10))
                    .show(ui, |ui| {
                        for file in &metadata.files {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(file.path.join("/"))
                                        .size(13.0)
                                        .color(theme.text),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            RichText::new(format_bytes(file.length))
                                                .size(12.0)
                                                .color(theme.text_dim)
                                                .monospace(),
                                        );
                                    },
                                );
                            });
                        }
                    });
            });
        });
    }

    fn show_log(&self, ui: &mut egui::Ui) {
        let theme = self.theme;
        card_frame(&theme).show(ui, |ui| {
            ui.label(
                RichText::new("Activity log")
                    .size(14.0)
                    .color(theme.text)
                    .strong(),
            );
            ui.add_space(8.0);

            Frame::new()
                .fill(theme.surface_alt)
                .stroke(Stroke::new(1.0, theme.border))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::same(10))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(160.0)
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            if self.logs.is_empty() {
                                ui.label(
                                    RichText::new("No activity yet.")
                                        .size(12.0)
                                        .color(theme.text_dim)
                                        .italics(),
                                );
                            } else {
                                for line in &self.logs {
                                    ui.label(
                                        RichText::new(line)
                                            .size(12.0)
                                            .color(theme.text_dim)
                                            .monospace(),
                                    );
                                }
                            }
                        });
                });
        });
    }
}

impl eframe::App for TorrentApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_worker(ctx);

        let theme = self.theme;
        egui::CentralPanel::default()
            .frame(Frame::new().fill(theme.bg).inner_margin(Margin::same(20)))
            .show(ctx, |ui| {
                self.show_header(ui, ctx);
                ui.add_space(16.0);
                self.show_toolbar(ui, ctx);
                ui.add_space(12.0);
                self.show_status(ui);
                ui.add_space(12.0);

                if self.metadata.is_some() {
                    self.show_torrent_details(ui);
                } else {
                    self.show_empty_state(ui);
                }

                ui.add_space(12.0);
                self.show_log(ui);
            });
    }
}

pub fn run() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("BitTorrent Client")
            .with_inner_size([960.0, 720.0])
            .with_min_inner_size([760.0, 560.0]),
        ..Default::default()
    };

    eframe::run_native(
        "BitTorrent Client",
        native_options,
        Box::new(|cc| {
            let app = TorrentApp::default();
            apply_theme(&cc.egui_ctx, &app.theme, app.dark_mode);
            Ok(Box::new(app))
        }),
    )
}
