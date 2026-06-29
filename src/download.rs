use std::net::SocketAddr;

use anyhow::{Context, Result};
use sha1::{Digest, Sha1};

use crate::peer::PeerSession;
use crate::storage::TorrentStorage;
use crate::torrent::TorrentMetadata;

pub fn download_torrent(
    metadata: &TorrentMetadata,
    peers: &[SocketAddr],
    output_dir: &std::path::Path,
    peer_id: [u8; 20],
) -> Result<()> {
    let storage = TorrentStorage::prepare(metadata, output_dir)?;
    let piece_count = metadata.piece_count();
    let mut downloaded_bytes = 0u64;
    let total_bytes = metadata.total_size();

    for piece_index in 0..piece_count {
        let index = piece_index as u32;
        let expected_hash = metadata.piece_hashes[piece_index];
        let length = crate::peer::piece_size(
            metadata.total_size(),
            metadata.piece_length,
            index,
        );

        println!(
            "Downloading piece {}/{} ({} bytes)...",
            piece_index + 1,
            piece_count,
            length
        );

        let piece_data = download_piece(metadata, peers, peer_id, index, length, expected_hash)
            .with_context(|| format!("failed to download piece {piece_index}"))?;

        storage
            .write_piece(index, metadata.piece_length, &piece_data)
            .with_context(|| format!("failed to write piece {piece_index} to disk"))?;

        downloaded_bytes += length;
        let percent = (downloaded_bytes as f64 / total_bytes as f64) * 100.0;
        println!(
            "Verified piece {piece_index} ({downloaded_bytes}/{total_bytes} bytes, {percent:.2}%)"
        );
    }

    println!("Download complete: {}", storage.root().display());
    Ok(())
}

fn download_piece(
    metadata: &TorrentMetadata,
    peers: &[SocketAddr],
    peer_id: [u8; 20],
    piece_index: u32,
    piece_length: u64,
    expected_hash: [u8; 20],
) -> Result<Vec<u8>> {
    let mut last_error = None;

    for peer in peers {
        match PeerSession::connect(*peer, metadata.info_hash, peer_id) {
            Ok(mut session) => {
                match session.download_piece(piece_index, piece_length, expected_hash) {
                    Ok(data) => return Ok(data),
                    Err(err) => {
                        eprintln!("peer {peer} failed on piece {piece_index}: {err:#}");
                        last_error = Some(err);
                    }
                }
            }
            Err(err) => {
                eprintln!("peer {peer} failed to connect: {err:#}");
                last_error = Some(err);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("no peers available for piece download")))
}

pub fn verify_piece(data: &[u8], expected_hash: [u8; 20]) -> Result<()> {
    let digest: [u8; 20] = Sha1::digest(data).into();
    if digest != expected_hash {
        anyhow::bail!("piece hash mismatch");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_piece_accepts_matching_hash() {
        let data = b"hello piece";
        let hash: [u8; 20] = Sha1::digest(data).into();
        verify_piece(data, hash).unwrap();
    }

    #[test]
    fn verify_piece_rejects_mismatch() {
        let err = verify_piece(b"bad", [0xFF; 20]).unwrap_err();
        assert!(err.to_string().contains("hash mismatch"));
    }
}
