use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use eframe::egui;

use crate::download::{self, DownloadProgress};
use crate::torrent::TorrentMetadata;
use crate::tracker::generate_peer_id;
use crate::{discover_peers, format_bytes, hex_encode, DiscoverConfig};

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
        }
    }
}

impl TorrentApp {
    fn busy(&self) -> bool {
        self.worker.is_some()
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
}

impl eframe::App for TorrentApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_worker(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("BitTorrent Client");
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                let busy = self.busy();
                if ui
                    .add_enabled(!busy, egui::Button::new("Open .torrent"))
                    .clicked()
                {
                    self.open_torrent_dialog();
                }
                if ui
                    .add_enabled(!busy && self.metadata.is_some(), egui::Button::new("Find peers"))
                    .clicked()
                {
                    self.start_find_peers(ctx);
                }
                if ui
                    .add_enabled(
                        !busy && self.metadata.is_some() && !self.peers.is_empty(),
                        egui::Button::new("Download"),
                    )
                    .clicked()
                {
                    self.start_download(ctx);
                }
                if ui
                    .add_enabled(!busy && self.metadata.is_some(), egui::Button::new("Output..."))
                    .clicked()
                {
                    self.pick_output_dir();
                }
            });

            ui.add_space(8.0);
            ui.checkbox(&mut self.use_dht, "Use DHT");
            ui.label(format!("Status: {}", self.status));

            if let Some(metadata) = &self.metadata {
                ui.add_space(12.0);
                ui.separator();
                ui.heading(&metadata.name);
                ui.label(format!(
                    "Info hash: {}",
                    hex_encode(metadata.info_hash)
                ));
                ui.label(format!(
                    "Size: {} ({} pieces)",
                    format_bytes(metadata.total_size()),
                    metadata.piece_count()
                ));
                ui.label(format!("Peers: {}", self.peers.len()));
                ui.label(format!(
                    "Output: {}",
                    self.output_dir.display()
                ));

                if let Some(path) = &self.torrent_path {
                    ui.label(format!("Torrent file: {}", path.display()));
                }

                ui.add_space(8.0);
                ui.collapsing("Files", |ui| {
                    for file in &metadata.files {
                        ui.label(format!(
                            "{} ({})",
                            file.path.join("/"),
                            format_bytes(file.length)
                        ));
                    }
                });

                if self.progress.total_bytes > 0
                    && (self.busy() || self.download_complete || self.progress.downloaded_bytes > 0)
                {
                    let fraction = self.progress.downloaded_bytes as f32
                        / self.progress.total_bytes as f32;
                    ui.add_space(8.0);
                    ui.add(
                        egui::ProgressBar::new(fraction).text(format!(
                            "{} / {} (piece {}/{})",
                            format_bytes(self.progress.downloaded_bytes),
                            format_bytes(self.progress.total_bytes),
                            self.progress.current_piece,
                            self.progress.total_pieces
                        )),
                    );
                }
            }

            ui.add_space(12.0);
            ui.separator();
            ui.label("Log");
            egui::ScrollArea::vertical()
                .max_height(180.0)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for line in &self.logs {
                        ui.label(line);
                    }
                });
        });
    }
}

pub fn run() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("BitTorrent Client")
            .with_inner_size([920.0, 680.0])
            .with_min_inner_size([720.0, 520.0]),
        ..Default::default()
    };

    eframe::run_native(
        "BitTorrent Client",
        native_options,
        Box::new(|_| Ok(Box::new(TorrentApp::default()))),
    )
}
