use std::fs::{self, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::torrent::TorrentMetadata;

#[derive(Debug, Clone)]
struct FileLayout {
    path: PathBuf,
    offset: u64,
    length: u64,
}

pub struct TorrentStorage {
    root: PathBuf,
    files: Vec<FileLayout>,
}

impl TorrentStorage {
    pub fn prepare(metadata: &TorrentMetadata, output_dir: impl AsRef<Path>) -> Result<Self> {
        let root = output_dir.as_ref().to_path_buf();
        fs::create_dir_all(&root)
            .with_context(|| format!("failed to create output directory {}", root.display()))?;

        let mut offset = 0;
        let mut files = Vec::with_capacity(metadata.files.len());

        for entry in &metadata.files {
            let path = root.join(entry.path.join("/"));
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("failed to create directory {}", parent.display())
                })?;
            }

            let file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&path)
                .with_context(|| format!("failed to create output file {}", path.display()))?;
            file.set_len(entry.length)
                .with_context(|| format!("failed to set size for {}", path.display()))?;

            files.push(FileLayout {
                path,
                offset,
                length: entry.length,
            });
            offset += entry.length;
        }

        Ok(Self { root, files })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn write_piece(
        &self,
        piece_index: u32,
        piece_length: u64,
        data: &[u8],
    ) -> Result<()> {
        let piece_start = piece_index as u64 * piece_length;
        let piece_end = piece_start + data.len() as u64;

        for file in &self.files {
            let file_end = file.offset + file.length;
            let write_start = piece_start.max(file.offset);
            let write_end = piece_end.min(file_end);
            if write_start >= write_end {
                continue;
            }

            let offset_in_piece = (write_start - piece_start) as usize;
            let offset_in_file = write_start - file.offset;
            let length = (write_end - write_start) as usize;

            let mut handle = OpenOptions::new()
                .write(true)
                .open(&file.path)
                .with_context(|| format!("failed to open {} for writing", file.path.display()))?;
            handle
                .seek(SeekFrom::Start(offset_in_file))
                .with_context(|| format!("failed to seek in {}", file.path.display()))?;
            handle
                .write_all(&data[offset_in_piece..offset_in_piece + length])
                .with_context(|| format!("failed to write piece to {}", file.path.display()))?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::torrent::{FileEntry, TorrentMetadata};

    #[test]
    fn writes_piece_bytes_to_correct_file_region() {
        let temp = std::env::temp_dir().join(format!(
            "bitorrentclient-storage-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp);

        let metadata = TorrentMetadata {
            announce: None,
            name: "dir".into(),
            piece_length: 8,
            piece_hashes: vec![[0; 20], [0; 20]],
            files: vec![
                FileEntry {
                    path: vec!["a.txt".into()],
                    length: 10,
                },
                FileEntry {
                    path: vec!["b.txt".into()],
                    length: 6,
                },
            ],
            info_hash: [0; 20],
        };

        let storage = TorrentStorage::prepare(&metadata, &temp).unwrap();
        storage
            .write_piece(0, metadata.piece_length, b"01234567")
            .unwrap();
        storage
            .write_piece(1, metadata.piece_length, b"89abcdef")
            .unwrap();

        let a = fs::read(temp.join("a.txt")).unwrap();
        let b = fs::read(temp.join("b.txt")).unwrap();
        assert_eq!(a, b"0123456789");
        assert_eq!(b, b"abcdef");

        let _ = fs::remove_dir_all(&temp);
    }
}
