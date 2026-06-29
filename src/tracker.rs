use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use bendy::decoding::{Decoder, Object};
use rand::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum AnnounceEvent {
    Started,
    Completed,
    Stopped,
}

impl AnnounceEvent {
    fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Completed => "completed",
            Self::Stopped => "stopped",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AnnounceRequest {
    pub info_hash: [u8; 20],
    pub peer_id: [u8; 20],
    pub port: u16,
    pub uploaded: u64,
    pub downloaded: u64,
    pub left: u64,
    pub event: Option<AnnounceEvent>,
}

#[derive(Debug, Clone)]
pub struct AnnounceResponse {
    pub interval: u32,
    pub peers: Vec<SocketAddr>,
    pub complete: Option<u32>,
    pub incomplete: Option<u32>,
}

pub fn generate_peer_id() -> [u8; 20] {
    let mut peer_id = [0u8; 20];
    peer_id[..8].copy_from_slice(b"-BT0001-");
    rand::thread_rng().fill(&mut peer_id[8..]);
    peer_id
}

pub fn announce(
    announce_url: &str,
    request: &AnnounceRequest,
) -> Result<AnnounceResponse> {
    if announce_url.starts_with("udp://") {
        bail!("UDP trackers are not supported yet (got {announce_url})");
    }
    if !announce_url.starts_with("http://") && !announce_url.starts_with("https://") {
        bail!("unsupported tracker URL scheme: {announce_url}");
    }

    let url = build_announce_url(announce_url, request)?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("failed to build HTTP client")?;

    let response = client
        .get(&url)
        .send()
        .with_context(|| format!("tracker announce request failed for {announce_url}"))?;

    let status = response.status();
    let body = response
        .bytes()
        .context("failed to read tracker response body")?;

    if !status.is_success() {
        bail!(
            "tracker returned HTTP {}: {}",
            status,
            String::from_utf8_lossy(&body)
        );
    }

    parse_announce_response(&body)
}

fn build_announce_url(announce_url: &str, request: &AnnounceRequest) -> Result<String> {
    let separator = if announce_url.contains('?') { '&' } else { '?' };

    let mut url = format!(
        "{announce_url}{separator}info_hash={}&peer_id={}&port={}&uploaded={}&downloaded={}&left={}&compact=1&numwant=50",
        percent_encode(&request.info_hash),
        percent_encode(&request.peer_id),
        request.port,
        request.uploaded,
        request.downloaded,
        request.left,
    );

    if let Some(event) = request.event {
        url.push_str("&event=");
        url.push_str(event.as_str());
    }

    Ok(url)
}

fn percent_encode(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("%{byte:02X}"))
        .collect()
}

pub fn parse_announce_response(body: &[u8]) -> Result<AnnounceResponse> {
    let mut decoder = Decoder::new(body);
    let root = map_bendy(decoder.next_object())?
        .ok_or_else(|| anyhow!("tracker response is empty"))?;

    let mut root_dict = map_bendy(root.try_into_dictionary())?;

    let mut interval = None;
    let mut peers = None;
    let mut complete = None;
    let mut incomplete = None;
    let mut failure_reason = None;

    while let Some((key, value)) = map_bendy(root_dict.next_pair())? {
        match key {
            b"interval" => interval = Some(parse_u32(value)?),
            b"peers" => peers = Some(parse_peers(value)?),
            b"complete" => complete = Some(parse_u32(value)?),
            b"incomplete" => incomplete = Some(parse_u32(value)?),
            b"failure reason" => {
                failure_reason = Some(parse_string(value)?);
            }
            _ => {}
        }
    }

    if let Some(reason) = failure_reason {
        bail!("tracker failure: {reason}");
    }

    let interval = interval.ok_or_else(|| anyhow!("tracker response missing interval"))?;
    let peers = peers.ok_or_else(|| anyhow!("tracker response missing peers"))?;

    Ok(AnnounceResponse {
        interval,
        peers,
        complete,
        incomplete,
    })
}

fn parse_peers(value: Object<'_, '_>) -> Result<Vec<SocketAddr>> {
    match value {
        Object::Bytes(bytes) => parse_compact_peers(bytes),
        Object::List(mut list) => {
            let mut peers = Vec::new();
            while let Some(entry) = map_bendy(list.next_object())? {
                peers.push(parse_dictionary_peer(entry)?);
            }
            Ok(peers)
        }
        _ => bail!("peers must be a byte string or list"),
    }
}

fn parse_compact_peers(bytes: &[u8]) -> Result<Vec<SocketAddr>> {
    if !bytes.len().is_multiple_of(6) {
        bail!(
            "compact peer list length must be a multiple of 6, got {}",
            bytes.len()
        );
    }

    let mut peers = Vec::with_capacity(bytes.len() / 6);
    for chunk in bytes.chunks_exact(6) {
        let ip = Ipv4Addr::new(chunk[0], chunk[1], chunk[2], chunk[3]);
        let port = u16::from_be_bytes([chunk[4], chunk[5]]);
        peers.push(SocketAddr::V4(SocketAddrV4::new(ip, port)));
    }

    Ok(peers)
}

fn parse_dictionary_peer(value: Object<'_, '_>) -> Result<SocketAddr> {
    let mut dict = map_bendy(value.try_into_dictionary())?;

    let mut ip = None;
    let mut port = None;

    while let Some((key, value)) = map_bendy(dict.next_pair())? {
        match key {
            b"ip" => ip = Some(parse_string(value)?),
            b"port" => port = Some(parse_u16(value)?),
            _ => {}
        }
    }

    let ip = ip.ok_or_else(|| anyhow!("peer dictionary missing ip"))?;
    let port = port.ok_or_else(|| anyhow!("peer dictionary missing port"))?;
    let addr = format!("{ip}:{port}")
        .parse()
        .with_context(|| format!("invalid peer address {ip}:{port}"))?;

    Ok(addr)
}

fn parse_string(value: Object<'_, '_>) -> Result<String> {
    let bytes = map_bendy(value.try_into_bytes())?;
    String::from_utf8(bytes.to_vec()).context("tracker string is not valid UTF-8")
}

fn parse_u32(value: Object<'_, '_>) -> Result<u32> {
    parse_u64(value)?.try_into().context("integer out of range for u32")
}

fn parse_u16(value: Object<'_, '_>) -> Result<u16> {
    parse_u64(value)?.try_into().context("integer out of range for u16")
}

fn parse_u64(value: Object<'_, '_>) -> Result<u64> {
    let digits = map_bendy(value.try_into_integer())?;
    digits
        .parse()
        .with_context(|| format!("invalid bencode integer: {digits}"))
}

fn map_bendy<T>(result: std::result::Result<T, bendy::decoding::Error>) -> Result<T> {
    result.map_err(|err| anyhow!("{err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_compact_peers() {
        let body = include_bytes!("../tests/fixtures/compact_response.bin");
        let response = parse_announce_response(body).unwrap();
        assert_eq!(response.interval, 60);
        assert_eq!(response.peers.len(), 1);
        assert_eq!(
            response.peers[0],
            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 6881))
        );
    }

    #[test]
    fn parses_dictionary_peers() {
        let body = include_bytes!("../tests/fixtures/dict_response.bin");
        let response = parse_announce_response(body).unwrap();
        assert_eq!(response.interval, 120);
        assert_eq!(response.peers.len(), 1);
        assert_eq!(
            response.peers[0],
            "192.168.1.100:51413".parse().unwrap()
        );
    }

    #[test]
    fn parses_failure_reason() {
        let body = include_bytes!("../tests/fixtures/failure_response.bin");
        let err = parse_announce_response(body).unwrap_err();
        assert!(err.to_string().contains("invalid info_hash"));
    }

    #[test]
    fn build_announce_url_encodes_binary_fields() {
        let request = AnnounceRequest {
            info_hash: [0xAB; 20],
            peer_id: [0xCD; 20],
            port: 6881,
            uploaded: 0,
            downloaded: 0,
            left: 1000,
            event: Some(AnnounceEvent::Started),
        };

        let url = build_announce_url("http://tracker/announce", &request).unwrap();
        assert!(url.contains("info_hash="));
        assert!(url.contains("%AB"));
        assert!(url.contains("event=started"));
        assert!(url.contains("compact=1"));
    }
}
