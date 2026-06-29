use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use sha1::Digest;

pub const BLOCK_SIZE: u32 = 16 * 1024;
const HANDSHAKE_LEN: u8 = 19;
const PROTOCOL: &[u8] = b"BitTorrent protocol";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const IO_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerMessage {
    KeepAlive,
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Have(u32),
    Bitfield(Vec<u8>),
    Request {
        index: u32,
        begin: u32,
        length: u32,
    },
    Piece {
        index: u32,
        begin: u32,
        block: Vec<u8>,
    },
    Cancel {
        index: u32,
        begin: u32,
        length: u32,
    },
    Unknown {
        id: u8,
        payload: Vec<u8>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadedBlock {
    pub piece_index: u32,
    pub begin: u32,
    pub data: Vec<u8>,
}

pub fn perform_handshake(
    stream: &mut TcpStream,
    info_hash: [u8; 20],
    peer_id: [u8; 20],
) -> Result<[u8; 20]> {
    write_handshake(stream, info_hash, peer_id)?;
    read_handshake(stream, info_hash)
}

pub fn download_block(
    addr: SocketAddr,
    info_hash: [u8; 20],
    peer_id: [u8; 20],
    piece_index: u32,
    begin: u32,
    length: u32,
) -> Result<DownloadedBlock> {
    let mut session = PeerSession::connect(addr, info_hash, peer_id)?;
    let data = session.request_block(piece_index, begin, length)?;
    Ok(DownloadedBlock {
        piece_index,
        begin,
        data,
    })
}

pub struct PeerSession {
    stream: TcpStream,
    addr: SocketAddr,
    choked: bool,
}

impl PeerSession {
    pub fn connect(addr: SocketAddr, info_hash: [u8; 20], peer_id: [u8; 20]) -> Result<Self> {
        let mut stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT)
            .with_context(|| format!("failed to connect to peer {addr}"))?;
        stream.set_read_timeout(Some(IO_TIMEOUT))?;
        stream.set_write_timeout(Some(IO_TIMEOUT))?;

        perform_handshake(&mut stream, info_hash, peer_id)
            .with_context(|| format!("handshake failed with peer {addr}"))?;

        let mut session = Self {
            stream,
            addr,
            choked: true,
        };
        session.send(PeerMessage::Interested)?;
        session.wait_unchoke()?;
        Ok(session)
    }

    pub fn download_piece(
        &mut self,
        piece_index: u32,
        piece_length: u64,
        expected_hash: [u8; 20],
    ) -> Result<Vec<u8>> {
        let mut data = vec![0u8; piece_length as usize];
        let mut begin = 0u32;

        while (begin as u64) < piece_length {
            let block_len = block_length_for_piece(piece_length, begin);
            let block = self.request_block(piece_index, begin, block_len)?;
            let start = begin as usize;
            data[start..start + block.len()].copy_from_slice(&block);
            begin += block_len;
        }

        let digest: [u8; 20] = sha1::Sha1::digest(&data).into();
        if digest != expected_hash {
            bail!("piece hash mismatch for piece {piece_index} from {}", self.addr);
        }

        Ok(data)
    }

    pub fn request_block(
        &mut self,
        piece_index: u32,
        begin: u32,
        length: u32,
    ) -> Result<Vec<u8>> {
        if self.choked {
            self.wait_unchoke()?;
        }

        self.send(PeerMessage::Request {
            index: piece_index,
            begin,
            length,
        })?;

        loop {
            match self.read()? {
                PeerMessage::Piece {
                    index,
                    begin: block_begin,
                    block,
                } if index == piece_index && block_begin == begin => {
                    if block.len() != length as usize {
                        bail!(
                            "expected block of {length} bytes, got {}",
                            block.len()
                        );
                    }
                    return Ok(block);
                }
                PeerMessage::Choke => {
                    self.choked = true;
                    self.wait_unchoke()?;
                    self.send(PeerMessage::Request {
                        index: piece_index,
                        begin,
                        length,
                    })?;
                }
                PeerMessage::Unchoke => self.choked = false,
                PeerMessage::KeepAlive
                | PeerMessage::Bitfield(_)
                | PeerMessage::Have(_)
                | PeerMessage::Interested
                | PeerMessage::NotInterested
                | PeerMessage::Unknown { .. } => {}
                other => {
                    bail!("unexpected message while waiting for piece: {other:?}");
                }
            }
        }
    }

    fn wait_unchoke(&mut self) -> Result<()> {
        if !self.choked {
            return Ok(());
        }

        loop {
            match self.read()? {
                PeerMessage::Unchoke => {
                    self.choked = false;
                    return Ok(());
                }
                PeerMessage::Choke => self.choked = true,
                PeerMessage::KeepAlive
                | PeerMessage::Bitfield(_)
                | PeerMessage::Have(_)
                | PeerMessage::Interested
                | PeerMessage::NotInterested
                | PeerMessage::Unknown { .. } => {}
                other => {
                    bail!("unexpected message while waiting for unchoke: {other:?}");
                }
            }
        }
    }

    fn send(&mut self, message: PeerMessage) -> Result<()> {
        send_message(&mut self.stream, message)
    }

    fn read(&mut self) -> Result<PeerMessage> {
        read_message(&mut self.stream)
    }
}

pub fn write_handshake<W: Write>(
    writer: &mut W,
    info_hash: [u8; 20],
    peer_id: [u8; 20],
) -> Result<()> {
    let mut buf = Vec::with_capacity(68);
    buf.push(HANDSHAKE_LEN);
    buf.extend_from_slice(PROTOCOL);
    buf.extend_from_slice(&[0u8; 8]);
    buf.extend_from_slice(&info_hash);
    buf.extend_from_slice(&peer_id);
    writer
        .write_all(&buf)
        .context("failed to write handshake")?;
    Ok(())
}

pub fn read_handshake<R: Read>(
    reader: &mut R,
    expected_info_hash: [u8; 20],
) -> Result<[u8; 20]> {
    let mut pstrlen = [0u8; 1];
    reader
        .read_exact(&mut pstrlen)
        .context("failed to read handshake length")?;
    if pstrlen[0] != HANDSHAKE_LEN {
        bail!("unexpected handshake protocol length: {}", pstrlen[0]);
    }

    let mut protocol = [0u8; 19];
    reader
        .read_exact(&mut protocol)
        .context("failed to read handshake protocol")?;
    if protocol != *PROTOCOL {
        bail!("unexpected handshake protocol");
    }

    let mut reserved = [0u8; 8];
    reader
        .read_exact(&mut reserved)
        .context("failed to read handshake reserved bytes")?;

    let mut info_hash = [0u8; 20];
    reader
        .read_exact(&mut info_hash)
        .context("failed to read handshake info hash")?;
    if info_hash != expected_info_hash {
        bail!("peer info hash does not match torrent");
    }

    let mut remote_peer_id = [0u8; 20];
    reader
        .read_exact(&mut remote_peer_id)
        .context("failed to read peer id")?;
    Ok(remote_peer_id)
}

pub fn send_message<W: Write>(writer: &mut W, message: PeerMessage) -> Result<()> {
    let payload = encode_message(&message)?;
    writer
        .write_all(&payload)
        .with_context(|| format!("failed to send peer message: {message:?}"))?;
    Ok(())
}

pub fn read_message<R: Read>(reader: &mut R) -> Result<PeerMessage> {
    let length = read_u32_be(reader).context("failed to read message length")?;
    if length == 0 {
        return Ok(PeerMessage::KeepAlive);
    }

    let mut buf = vec![0u8; length as usize];
    reader
        .read_exact(&mut buf)
        .context("failed to read message body")?;
    decode_message(&buf)
}

fn encode_message(message: &PeerMessage) -> Result<Vec<u8>> {
    match message {
        PeerMessage::KeepAlive => Ok(vec![0, 0, 0, 0]),
        PeerMessage::Choke => Ok(encode_payload(0, &[])?),
        PeerMessage::Unchoke => Ok(encode_payload(1, &[])?),
        PeerMessage::Interested => Ok(encode_payload(2, &[])?),
        PeerMessage::NotInterested => Ok(encode_payload(3, &[])?),
        PeerMessage::Have(index) => Ok(encode_payload(4, &index.to_be_bytes())?),
        PeerMessage::Bitfield(bits) => Ok(encode_payload(5, bits)?),
        PeerMessage::Request {
            index,
            begin,
            length,
        } => {
            let mut payload = Vec::with_capacity(12);
            payload.extend_from_slice(&index.to_be_bytes());
            payload.extend_from_slice(&begin.to_be_bytes());
            payload.extend_from_slice(&length.to_be_bytes());
            Ok(encode_payload(6, &payload)?)
        }
        PeerMessage::Piece {
            index,
            begin,
            block,
        } => {
            let mut payload = Vec::with_capacity(8 + block.len());
            payload.extend_from_slice(&index.to_be_bytes());
            payload.extend_from_slice(&begin.to_be_bytes());
            payload.extend_from_slice(block);
            Ok(encode_payload(7, &payload)?)
        }
        PeerMessage::Cancel {
            index,
            begin,
            length,
        } => {
            let mut payload = Vec::with_capacity(12);
            payload.extend_from_slice(&index.to_be_bytes());
            payload.extend_from_slice(&begin.to_be_bytes());
            payload.extend_from_slice(&length.to_be_bytes());
            Ok(encode_payload(8, &payload)?)
        }
        PeerMessage::Unknown { .. } => bail!("cannot send unknown peer message"),
    }
}

fn encode_payload(id: u8, payload: &[u8]) -> Result<Vec<u8>> {
    let length = 1 + payload.len();
    if length > u32::MAX as usize {
        bail!("peer message too large");
    }

    let mut buf = Vec::with_capacity(4 + length);
    buf.extend_from_slice(&(length as u32).to_be_bytes());
    buf.push(id);
    buf.extend_from_slice(payload);
    Ok(buf)
}

fn decode_message(buf: &[u8]) -> Result<PeerMessage> {
    let Some((&id, payload)) = buf.split_first() else {
        bail!("empty peer message body");
    };

    Ok(match id {
        0 => PeerMessage::Choke,
        1 => PeerMessage::Unchoke,
        2 => PeerMessage::Interested,
        3 => PeerMessage::NotInterested,
        4 => {
            let index = read_u32_from_payload(payload, "have")?;
            PeerMessage::Have(index)
        }
        5 => PeerMessage::Bitfield(payload.to_vec()),
        6 => {
            if payload.len() != 12 {
                bail!("invalid request message length: {}", payload.len());
            }
            PeerMessage::Request {
                index: u32::from_be_bytes(payload[0..4].try_into()?),
                begin: u32::from_be_bytes(payload[4..8].try_into()?),
                length: u32::from_be_bytes(payload[8..12].try_into()?),
            }
        }
        7 => {
            if payload.len() < 8 {
                bail!("invalid piece message length: {}", payload.len());
            }
            PeerMessage::Piece {
                index: u32::from_be_bytes(payload[0..4].try_into()?),
                begin: u32::from_be_bytes(payload[4..8].try_into()?),
                block: payload[8..].to_vec(),
            }
        }
        8 => {
            if payload.len() != 12 {
                bail!("invalid cancel message length: {}", payload.len());
            }
            PeerMessage::Cancel {
                index: u32::from_be_bytes(payload[0..4].try_into()?),
                begin: u32::from_be_bytes(payload[4..8].try_into()?),
                length: u32::from_be_bytes(payload[8..12].try_into()?),
            }
        }
        other => PeerMessage::Unknown {
            id: other,
            payload: payload.to_vec(),
        },
    })
}

fn read_u32_from_payload(payload: &[u8], message: &str) -> Result<u32> {
    let bytes: [u8; 4] = payload
        .get(..4)
        .ok_or_else(|| anyhow!("{message} message payload too short"))?
        .try_into()?;
    Ok(u32::from_be_bytes(bytes))
}

fn read_u32_be<R: Read>(reader: &mut R) -> Result<u32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(u32::from_be_bytes(buf))
}

pub fn block_length_for_piece(piece_size: u64, begin: u32) -> u32 {
    let remaining = piece_size.saturating_sub(begin as u64);
    remaining.min(BLOCK_SIZE as u64) as u32
}

pub fn piece_size(total_size: u64, piece_length: u64, piece_index: u32) -> u64 {
    let start = piece_index as u64 * piece_length;
    if start >= total_size {
        return 0;
    }
    let end = (start + piece_length).min(total_size);
    end - start
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn handshake_roundtrip() {
        let info_hash = [0xAB; 20];
        let peer_id = [0xCD; 20];

        let mut buf = Vec::new();
        write_handshake(&mut buf, info_hash, peer_id).unwrap();

        let mut cursor = Cursor::new(buf);
        let remote = read_handshake(&mut cursor, info_hash).unwrap();
        assert_eq!(remote, peer_id);
    }

    #[test]
    fn handshake_rejects_mismatched_info_hash() {
        let info_hash = [0xAB; 20];
        let peer_id = [0xCD; 20];

        let mut buf = Vec::new();
        write_handshake(&mut buf, info_hash, peer_id).unwrap();

        let mut cursor = Cursor::new(buf);
        let err = read_handshake(&mut cursor, [0xFF; 20]).unwrap_err();
        assert!(err.to_string().contains("info hash"));
    }

    #[test]
    fn encodes_and_decodes_request_message() {
        let encoded = encode_message(&PeerMessage::Request {
            index: 3,
            begin: 0,
            length: 16384,
        })
        .unwrap();
        assert_eq!(&encoded[0..4], &(13u32.to_be_bytes()));
        let decoded = decode_message(&encoded[4..]).unwrap();
        assert_eq!(
            decoded,
            PeerMessage::Request {
                index: 3,
                begin: 0,
                length: 16384,
            }
        );
    }

    #[test]
    fn encodes_and_decodes_piece_message() {
        let encoded = encode_message(&PeerMessage::Piece {
            index: 0,
            begin: 0,
            block: vec![1, 2, 3, 4],
        })
        .unwrap();
        let decoded = decode_message(&encoded[4..]).unwrap();
        assert_eq!(
            decoded,
            PeerMessage::Piece {
                index: 0,
                begin: 0,
                block: vec![1, 2, 3, 4],
            }
        );
    }

    #[test]
    fn block_length_for_last_partial_piece() {
        assert_eq!(block_length_for_piece(14, 0), 14);
        assert_eq!(block_length_for_piece(16384, 0), 16384);
        assert_eq!(block_length_for_piece(20000, 16384), 3616);
    }
}
