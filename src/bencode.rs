use anyhow::{anyhow, bail, Context, Result};
use sha1::{Digest, Sha1};

/// Returns the byte length of one complete bencode value starting at `data[0]`.
pub fn bencode_value_len(data: &[u8]) -> Result<usize> {
    if data.is_empty() {
        bail!("empty bencode input");
    }

    match data[0] {
        b'i' => {
            let end = data
                .iter()
                .position(|&b| b == b'e')
                .context("unterminated bencode integer")?;
            Ok(end + 1)
        }
        b'l' => {
            let mut offset = 1;
            while data.get(offset).is_some_and(|&b| b != b'e') {
                offset += bencode_value_len(&data[offset..])?;
            }
            if data.get(offset) != Some(&b'e') {
                bail!("unterminated bencode list");
            }
            Ok(offset + 1)
        }
        b'd' => {
            let mut offset = 1;
            while data.get(offset).is_some_and(|&b| b != b'e') {
                offset += bencode_value_len(&data[offset..])?;
                offset += bencode_value_len(&data[offset..])?;
            }
            if data.get(offset) != Some(&b'e') {
                bail!("unterminated bencode dictionary");
            }
            Ok(offset + 1)
        }
        b'0'..=b'9' => {
            let colon = data
                .iter()
                .position(|&b| b == b':')
                .context("invalid bencode string length")?;
            let len: usize = std::str::from_utf8(&data[..colon])
                .context("invalid bencode string length digits")?
                .parse()
                .context("invalid bencode string length value")?;
            Ok(colon + 1 + len)
        }
        byte => bail!("unexpected bencode prefix: {byte}"),
    }
}

/// Locates and returns the raw bencode bytes of the torrent `info` dictionary.
pub fn info_dict_bytes(data: &[u8]) -> Result<&[u8]> {
    let needle = b"4:info";
    let pos = data
        .windows(needle.len())
        .position(|window| window == needle)
        .ok_or_else(|| anyhow!("torrent file is missing the info dictionary"))?;
    let start = pos + needle.len();
    let len = bencode_value_len(&data[start..])?;
    Ok(&data[start..start + len])
}

/// Computes the 20-byte info hash (SHA-1 of the raw `info` dictionary bytes).
pub fn info_hash(data: &[u8]) -> Result<[u8; 20]> {
    let digest = Sha1::digest(info_dict_bytes(data)?);
    Ok(digest.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_hash_matches_info_dict_sha1() {
        let data = br#"d8:announce14:http://tracker4:infod6:lengthi1000e4:name4:test12:piece lengthi32768e6:pieces20:aaaaaaaaaaaaaaaaaaaaee"#;
        let hash = info_hash(data).unwrap();
        let expected: [u8; 20] = Sha1::digest(info_dict_bytes(data).unwrap()).into();
        assert_eq!(hash, expected);
    }
}
