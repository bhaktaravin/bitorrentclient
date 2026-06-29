use std::fs;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use bendy::decoding::{Decoder, Object};

use crate::bencode;

fn map_bendy<T>(result: std::result::Result<T, bendy::decoding::Error>) -> Result<T> {
    result.map_err(|err| anyhow!("{err}"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub path: Vec<String>,
    pub length: u64,
}

#[derive(Debug, Clone)]
pub struct TorrentMetadata {
    pub announce: Option<String>,
    pub name: String,
    pub piece_length: u64,
    pub piece_hashes: Vec<[u8; 20]>,
    pub files: Vec<FileEntry>,
    pub info_hash: [u8; 20],
}

impl TorrentMetadata {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let bytes = fs::read(path.as_ref())
            .with_context(|| format!("failed to read {}", path.as_ref().display()))?;
        Self::from_bytes(&bytes)
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let info_hash = bencode::info_hash(data)?;

        let mut decoder = Decoder::new(data);
        let root = map_bendy(decoder.next_object())?
            .ok_or_else(|| anyhow!("torrent file is empty"))?;

        let mut root_dict = map_bendy(root.try_into_dictionary())?;

        let mut announce = None;
        let mut info = None;

        while let Some((key, value)) = map_bendy(root_dict.next_pair())? {
            match key {
                b"announce" => announce = Some(parse_string(value)?),
                b"info" => info = Some(parse_info(value)?),
                _ => {}
            }
        }

        let info = info.ok_or_else(|| anyhow!("torrent is missing info dictionary"))?;

        Ok(Self {
            announce,
            name: info.name,
            piece_length: info.piece_length,
            piece_hashes: info.piece_hashes,
            files: info.files,
            info_hash,
        })
    }

    pub fn total_size(&self) -> u64 {
        self.files.iter().map(|file| file.length).sum()
    }

    pub fn piece_count(&self) -> usize {
        self.piece_hashes.len()
    }
}

struct ParsedInfo {
    name: String,
    piece_length: u64,
    piece_hashes: Vec<[u8; 20]>,
    files: Vec<FileEntry>,
}

fn parse_info(value: Object<'_, '_>) -> Result<ParsedInfo> {
    let mut info_dict = map_bendy(value.try_into_dictionary())?;

    let mut name = None;
    let mut piece_length = None;
    let mut pieces = None;
    let mut single_length = None;
    let mut file_list = None;

    while let Some((key, value)) = map_bendy(info_dict.next_pair())? {
        match key {
            b"name" => name = Some(parse_string(value)?),
            b"piece length" => piece_length = Some(parse_integer(value)?),
            b"pieces" => pieces = Some(parse_bytes(value)?),
            b"length" => single_length = Some(parse_integer(value)?),
            b"files" => file_list = Some(parse_files(value)?),
            _ => {}
        }
    }

    let name = name.ok_or_else(|| anyhow!("torrent info is missing name"))?;
    let piece_length =
        piece_length.ok_or_else(|| anyhow!("torrent info is missing piece length"))?;

    let pieces = pieces.ok_or_else(|| anyhow!("torrent info is missing piece hashes"))?;
    if !pieces.len().is_multiple_of(20) {
        bail!(
            "piece hash blob length must be a multiple of 20, got {}",
            pieces.len()
        );
    }

    let piece_hashes = pieces
        .chunks_exact(20)
        .map(|chunk| chunk.try_into().expect("chunk is 20 bytes"))
        .collect();

    let files = if let Some(length) = single_length {
        vec![FileEntry {
            path: vec![name.clone()],
            length,
        }]
    } else if let Some(files) = file_list {
        files
    } else {
        bail!("torrent info must contain either length or files");
    };

    Ok(ParsedInfo {
        name,
        piece_length,
        piece_hashes,
        files,
    })
}

fn parse_files(value: Object<'_, '_>) -> Result<Vec<FileEntry>> {
    let mut list = map_bendy(value.try_into_list())?;
    let mut files = Vec::new();

    while let Some(entry) = map_bendy(list.next_object())? {
        files.push(parse_file_entry(entry)?);
    }

    Ok(files)
}

fn parse_file_entry(value: Object<'_, '_>) -> Result<FileEntry> {
    let mut dict = map_bendy(value.try_into_dictionary())?;

    let mut length = None;
    let mut path = None;

    while let Some((key, value)) = map_bendy(dict.next_pair())? {
        match key {
            b"length" => length = Some(parse_integer(value)?),
            b"path" => path = Some(parse_path(value)?),
            _ => {}
        }
    }

    let length = length.ok_or_else(|| anyhow!("file entry is missing length"))?;
    let path = path.ok_or_else(|| anyhow!("file entry is missing path"))?;

    Ok(FileEntry { path, length })
}

fn parse_path(value: Object<'_, '_>) -> Result<Vec<String>> {
    let mut list = map_bendy(value.try_into_list())?;
    let mut parts = Vec::new();

    while let Some(entry) = map_bendy(list.next_object())? {
        parts.push(parse_string(entry)?);
    }

    Ok(parts)
}

fn parse_string(value: Object<'_, '_>) -> Result<String> {
    let bytes = map_bendy(value.try_into_bytes())?;
    String::from_utf8(bytes.to_vec()).context("torrent string is not valid UTF-8")
}

fn parse_bytes(value: Object<'_, '_>) -> Result<Vec<u8>> {
    Ok(map_bendy(value.try_into_bytes())?.to_vec())
}

fn parse_integer(value: Object<'_, '_>) -> Result<u64> {
    let digits = map_bendy(value.try_into_integer())?;
    digits
        .parse()
        .with_context(|| format!("invalid bencode integer: {digits}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SINGLE_FILE: &[u8] =
        br#"d8:announce14:http://tracker4:infod6:lengthi1000e4:name4:test12:piece lengthi32768e6:pieces20:aaaaaaaaaaaaaaaaaaaaee"#;

    #[test]
    fn parses_single_file_torrent() {
        let torrent = TorrentMetadata::from_bytes(SINGLE_FILE).unwrap();
        assert_eq!(torrent.name, "test");
        assert_eq!(torrent.announce.as_deref(), Some("http://tracker"));
        assert_eq!(torrent.piece_length, 32_768);
        assert_eq!(torrent.piece_count(), 1);
        assert_eq!(torrent.total_size(), 1000);
        assert_eq!(torrent.files.len(), 1);
        assert_eq!(torrent.files[0].path, vec!["test".to_string()]);
    }

    #[test]
    fn parses_multi_file_torrent() {
        let data = include_bytes!("../tests/fixtures/multi.torrent");
        let torrent = TorrentMetadata::from_bytes(data).unwrap();
        assert_eq!(torrent.name, "dir");
        assert_eq!(torrent.announce.as_deref(), Some("http://tracker"));
        assert_eq!(torrent.piece_count(), 2);
        assert_eq!(torrent.total_size(), 300);
        assert_eq!(torrent.files.len(), 2);
        assert_eq!(torrent.files[0].path, vec!["a.txt".to_string()]);
        assert_eq!(torrent.files[1].path, vec!["b.txt".to_string()]);
    }
}
