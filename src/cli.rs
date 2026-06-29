use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;

use crate::download;
use crate::peer::{block_length_for_piece, download_block, piece_size};
use crate::torrent::TorrentMetadata;
use crate::tracker::generate_peer_id;
use crate::{discover_peers, hex_encode, DiscoverConfig};

#[derive(Parser)]
#[command(name = "bitorrentclient", about = "A BitTorrent client written in Rust")]
pub struct Cli {
    /// Path to a .torrent file
    pub torrent: PathBuf,

    /// TCP port to report to the tracker
    #[arg(long, default_value_t = 6881)]
    pub port: u16,

    /// Skip contacting the tracker
    #[arg(long)]
    pub no_announce: bool,

    /// Skip DHT peer lookup
    #[arg(long)]
    pub no_dht: bool,

    /// Download the complete torrent
    #[arg(long)]
    pub download: bool,

    /// Output directory for --download
    #[arg(long, value_name = "DIR")]
    pub output: Option<PathBuf>,

    /// Connect to a peer and download the first block of piece 0
    #[arg(long)]
    pub connect: bool,

    /// Peer address to connect to (skips picking one from the tracker/DHT)
    #[arg(long)]
    pub peer: Option<SocketAddr>,
}

pub fn run() -> Result<()> {
    run_from(std::env::args_os())
}

pub fn run_from<I, T>(args: I) -> Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = Cli::parse_from(args);
    let metadata = TorrentMetadata::from_path(&cli.torrent)?;
    let peer_id = generate_peer_id();

    println!("Name:         {}", metadata.name);
    match &metadata.announce {
        Some(url) => println!("Announce:     {url}"),
        None => println!("Announce:     (none — DHT/magnet only)"),
    }
    println!("Info hash:    {}", hex_encode(metadata.info_hash));
    println!("Total size:   {} bytes", metadata.total_size());
    println!("Piece length: {} bytes", metadata.piece_length);
    println!("Piece count:  {}", metadata.piece_count());
    println!("Files:");

    for file in &metadata.files {
        let path = file.path.join("/");
        println!("  - {} ({} bytes)", path, file.length);
    }

    let wants_download = cli.download;
    let wants_connect = cli.connect || cli.peer.is_some();

    if wants_download && wants_connect {
        bail!("use either --download or --connect, not both");
    }

    let needs_peers = wants_download || wants_connect;
    let mut peers = Vec::new();

    if !cli.no_announce {
        if metadata.announce.is_some() {
            println!();
            println!("Announcing to tracker...");

            peers = discover_peers(
                &metadata,
                peer_id,
                &DiscoverConfig {
                    use_tracker: true,
                    use_dht: false,
                    port: cli.port,
                },
            )?;

            println!("Peers:        {}", peers.len());
            for peer in &peers {
                println!("  - {peer}");
            }
        } else {
            println!();
            println!("No HTTP tracker in this torrent.");
        }
    }

    if peers.is_empty() && !cli.no_dht && (!cli.no_announce || needs_peers) {
        println!();
        println!("Querying DHT for peers...");
        peers = discover_peers(
            &metadata,
            peer_id,
            &DiscoverConfig {
                use_tracker: false,
                use_dht: true,
                port: cli.port,
            },
        )?;
        println!("DHT peers:    {}", peers.len());
        if !wants_download {
            for peer in &peers {
                println!("  - {peer}");
            }
        }
    }

    if wants_download {
        if cli.peer.is_none() && peers.is_empty() {
            bail!("no peers found via tracker or DHT; try again or pass --peer");
        }

        let output_dir = cli
            .output
            .unwrap_or_else(|| PathBuf::from(&metadata.name));

        println!();
        println!("Downloading to {}...", output_dir.display());

        let peer_list: Vec<SocketAddr> = if let Some(addr) = cli.peer {
            vec![addr]
        } else {
            peers
        };

        download::download_torrent(&metadata, &peer_list, &output_dir, peer_id, None)?;
        println!("Download complete: {}", output_dir.display());
        return Ok(());
    }

    if !wants_connect {
        return Ok(());
    }

    if cli.peer.is_none() && peers.is_empty() {
        bail!("no peers found via tracker or DHT; try again or pass --peer");
    }

    let piece_index = 0;
    let begin = 0;
    let piece_len = piece_size(
        metadata.total_size(),
        metadata.piece_length,
        piece_index,
    );
    if piece_len == 0 {
        bail!("torrent has no data to download");
    }
    let block_len = block_length_for_piece(piece_len, begin);

    println!();
    println!(
        "Requesting piece {piece_index}, offset {begin}, length {block_len} bytes"
    );

    let mut last_error = None;
    let peer_list: Vec<SocketAddr> = if let Some(addr) = cli.peer {
        vec![addr]
    } else {
        peers
    };

    for addr in peer_list {
        println!("Connecting to peer {addr}...");
        match download_block(
            addr,
            metadata.info_hash,
            peer_id,
            piece_index,
            begin,
            block_len,
        ) {
            Ok(block) => {
                println!("Downloaded {} bytes", block.data.len());
                if block.data.is_ascii() {
                    println!("Data (ascii): {}", String::from_utf8_lossy(&block.data));
                } else {
                    println!("Data (hex):   {}", hex_encode(&block.data));
                }
                return Ok(());
            }
            Err(err) => {
                eprintln!("peer {addr} failed: {err:#}");
                last_error = Some(err);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow!("no peers available to connect to")))
        .context("failed to download block from any peer")
}
