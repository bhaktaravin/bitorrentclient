pub mod bencode;
pub mod cli;
pub mod dht;
pub mod download;
pub mod gui;
pub mod peer;
pub mod storage;
pub mod torrent;
pub mod tracker;

use std::net::SocketAddr;

use anyhow::{Context, Result};

use torrent::TorrentMetadata;
use tracker::{AnnounceEvent, AnnounceRequest};

pub fn hex_encode(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;

    if bytes >= GIB {
        format!("{:.2} GiB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.2} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.2} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} B")
    }
}

#[derive(Debug, Clone)]
pub struct DiscoverConfig {
    pub use_tracker: bool,
    pub use_dht: bool,
    pub port: u16,
}

impl Default for DiscoverConfig {
    fn default() -> Self {
        Self {
            use_tracker: true,
            use_dht: true,
            port: 6881,
        }
    }
}

pub fn discover_peers(
    metadata: &TorrentMetadata,
    peer_id: [u8; 20],
    config: &DiscoverConfig,
) -> Result<Vec<SocketAddr>> {
    let mut peers = Vec::new();

    if config.use_tracker {
        if let Some(announce_url) = &metadata.announce {
            let request = AnnounceRequest {
                info_hash: metadata.info_hash,
                peer_id,
                port: config.port,
                uploaded: 0,
                downloaded: 0,
                left: metadata.total_size(),
                event: Some(AnnounceEvent::Started),
            };
            let response = tracker::announce(announce_url, &request)?;
            peers = response.peers;
        }
    }

    if peers.is_empty() && config.use_dht {
        peers = dht::find_peers(metadata.info_hash).context("DHT lookup failed")?;
    }

    Ok(peers)
}
