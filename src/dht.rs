use std::collections::{HashSet, VecDeque};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use rand::Rng;

use crate::bencode::bencode_value_len;

const BOOTSTRAP_NODES: &[&str] = &[
    "router.bittorrent.com:6881",
    "router.utorrent.com:6881",
    "dht.transmissionbt.com:6881",
    "dht.libtorrent.org:25401",
];

const MAX_QUERIES: usize = 48;
const QUERY_TIMEOUT: Duration = Duration::from_secs(3);
const LOOKUP_BUDGET: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct NodeId([u8; 20]);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DhtNode {
    id: NodeId,
    addr: SocketAddr,
}

#[derive(Debug, Clone, Default)]
struct QueryResponse {
    responder_id: Option<[u8; 20]>,
    nodes: Vec<DhtNode>,
    peers: Vec<SocketAddr>,
}

pub fn generate_node_id() -> [u8; 20] {
    let mut node_id = [0u8; 20];
    node_id[..8].copy_from_slice(b"-BT0001-");
    rand::thread_rng().fill(&mut node_id[8..]);
    node_id
}

pub fn find_peers(info_hash: [u8; 20]) -> Result<Vec<SocketAddr>> {
    let socket = UdpSocket::bind("0.0.0.0:0").context("failed to bind UDP socket for DHT")?;
    socket
        .set_read_timeout(Some(QUERY_TIMEOUT))
        .context("failed to set DHT socket timeout")?;

    let node_id = generate_node_id();
    let mut client = DhtClient {
        socket,
        node_id,
        target: info_hash,
    };

    client.lookup_peers()
}

struct DhtClient {
    socket: UdpSocket,
    node_id: [u8; 20],
    target: [u8; 20],
}

impl DhtClient {
    fn lookup_peers(&mut self) -> Result<Vec<SocketAddr>> {
        let deadline = Instant::now() + LOOKUP_BUDGET;
        let mut queried = HashSet::new();
        let mut peers = HashSet::new();
        let mut queue: VecDeque<SocketAddr> = resolve_bootstrap_nodes()?.into();
        let mut known_nodes: Vec<DhtNode> = Vec::new();

        while let Some(addr) = queue.pop_front() {
            if Instant::now() >= deadline || queried.len() >= MAX_QUERIES {
                break;
            }
            if !queried.insert(addr) {
                continue;
            }

            let response = match self.query_get_peers(addr, deadline) {
                Ok(response) => response,
                Err(_) => continue,
            };

            if let Some(id) = response.responder_id {
                known_nodes.push(DhtNode {
                    id: NodeId(id),
                    addr,
                });
            }

            for peer in response.peers {
                peers.insert(peer);
            }

            for node in response.nodes {
                if !queried.contains(&node.addr) {
                    known_nodes.push(node.clone());
                    queue.push_back(node.addr);
                }
            }

            if peers.len() >= 20 {
                break;
            }

            sort_queue_by_distance(&mut queue, &known_nodes, &self.target);
        }

        if peers.is_empty() {
            bail!("DHT lookup returned no peers");
        }

        Ok(peers.into_iter().collect())
    }

    fn query_get_peers(&self, addr: SocketAddr, deadline: Instant) -> Result<QueryResponse> {
        let tx = random_transaction_id();
        let packet = encode_query(&tx, "get_peers", |args| {
            args.extend(encode_dict_pair(b"id", &encode_bytes(&self.node_id)));
            args.extend(encode_dict_pair(
                b"info_hash",
                &encode_bytes(&self.target),
            ));
            Ok(())
        })?;
        self.send_and_wait(addr, &packet, &tx, deadline)
    }

    fn send_and_wait(
        &self,
        addr: SocketAddr,
        packet: &[u8],
        tx: &[u8],
        deadline: Instant,
    ) -> Result<QueryResponse> {
        self.socket
            .send_to(packet, addr)
            .with_context(|| format!("failed to send DHT query to {addr}"))?;

        let mut buffer = [0u8; 1500];
        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            self.socket
                .set_read_timeout(Some(remaining.min(QUERY_TIMEOUT)))
                .ok();

            match self.socket.recv_from(&mut buffer) {
                Ok((size, from)) => {
                    if from != addr {
                        continue;
                    }
                    match decode_response(&buffer[..size], tx) {
                        Ok(response) => return Ok(response),
                        Err(err) => {
                            eprintln!("ignored malformed DHT response from {from}: {err:#}");
                        }
                    }
                }
                Err(err)
                    if err.kind() == std::io::ErrorKind::WouldBlock
                        || err.kind() == std::io::ErrorKind::TimedOut =>
                {
                    break;
                }
                Err(err) => return Err(err.into()),
            }
        }

        bail!("timed out waiting for DHT response from {addr}")
    }
}

fn sort_queue_by_distance(
    queue: &mut VecDeque<SocketAddr>,
    known_nodes: &[DhtNode],
    target: &[u8; 20],
) {
    let mut items: Vec<SocketAddr> = queue.drain(..).collect();
    items.sort_by_key(|addr| {
        known_nodes
            .iter()
            .find(|node| node.addr == *addr)
            .map(|node| distance(target, &node.id.0))
            .unwrap_or([0xFF; 20])
    });
    queue.extend(items);
}

fn resolve_bootstrap_nodes() -> Result<Vec<SocketAddr>> {
    let mut addrs = Vec::new();
    for host in BOOTSTRAP_NODES {
        let resolved = host
            .to_socket_addrs()
            .with_context(|| format!("failed to resolve DHT bootstrap node {host}"))?;
        addrs.extend(resolved);
    }
    if addrs.is_empty() {
        bail!("no bootstrap DHT nodes resolved");
    }
    Ok(addrs)
}

fn distance(left: &[u8; 20], right: &[u8; 20]) -> [u8; 20] {
    let mut out = [0u8; 20];
    for (index, byte) in out.iter_mut().enumerate() {
        *byte = left[index] ^ right[index];
    }
    out
}

fn random_transaction_id() -> Vec<u8> {
    let mut tx = vec![0u8; 2];
    rand::thread_rng().fill(&mut tx[..]);
    tx
}

fn encode_query(
    tx: &[u8],
    method: &str,
    fill_args: impl FnOnce(&mut Vec<u8>) -> Result<()>,
) -> Result<Vec<u8>> {
    let mut args = Vec::new();
    args.push(b'd');
    fill_args(&mut args)?;
    args.push(b'e');

    let mut message = Vec::new();
    message.push(b'd');
    message.extend(encode_dict_pair(b"a", &args));
    message.extend(encode_dict_pair(b"q", &encode_string(method.as_bytes())));
    message.extend(encode_dict_pair(b"t", &encode_bytes(tx)));
    message.extend(encode_dict_pair(b"y", &encode_string(b"q")));
    message.push(b'e');
    Ok(message)
}

fn encode_dict_pair(key: &[u8], encoded_value: &[u8]) -> Vec<u8> {
    let mut out = encode_string(key);
    out.extend_from_slice(encoded_value);
    out
}

fn encode_string(value: &[u8]) -> Vec<u8> {
    let mut out = value.len().to_string().into_bytes();
    out.push(b':');
    out.extend_from_slice(value);
    out
}

fn encode_bytes(value: &[u8]) -> Vec<u8> {
    encode_string(value)
}

fn decode_response(packet: &[u8], expected_tx: &[u8]) -> Result<QueryResponse> {
    let root = parse_bencode(packet)?;
    let BencodeValue::Dict(entries) = root else {
        bail!("DHT response root must be a dictionary");
    };

    let mut tx = None;
    let mut message_type = None;
    let mut response = None;
    let mut error = None;

    for (key, value) in entries {
        match key.as_slice() {
            b"t" => tx = Some(value.as_bytes()?.to_vec()),
            b"y" => message_type = Some(value.as_bytes()?.to_vec()),
            b"r" => response = Some(parse_response_body(value)?),
            b"e" => error = Some(parse_error(value)?),
            _ => {}
        }
    }

    let tx = tx.ok_or_else(|| anyhow!("DHT response missing transaction id"))?;
    if tx != expected_tx {
        bail!("unexpected DHT transaction id");
    }

    if let Some(message) = error {
        bail!("DHT error {}: {}", message.0, message.1);
    }

    let message_type = message_type.ok_or_else(|| anyhow!("DHT response missing type"))?;
    if message_type != b"r" {
        bail!(
            "expected DHT response, got {:?}",
            String::from_utf8_lossy(&message_type)
        );
    }

    response.ok_or_else(|| anyhow!("DHT response missing body"))
}

fn parse_response_body(value: BencodeValue) -> Result<QueryResponse> {
    let BencodeValue::Dict(entries) = value else {
        bail!("DHT response body must be a dictionary");
    };

    let mut result = QueryResponse::default();
    for (key, value) in entries {
        match key.as_slice() {
            b"id" => {
                let id = value.as_bytes()?;
                if id.len() == 20 {
                    let mut responder = [0u8; 20];
                    responder.copy_from_slice(id);
                    result.responder_id = Some(responder);
                }
            }
            b"nodes" => {
                result.nodes.extend(parse_compact_nodes(value.as_bytes()?)?);
            }
            b"values" => {
                result.peers.extend(parse_values_list(value)?);
            }
            b"peers" => {
                result.peers.extend(parse_compact_peers(value.as_bytes()?)?);
            }
            _ => {}
        }
    }

    Ok(result)
}

fn parse_values_list(value: BencodeValue) -> Result<Vec<SocketAddr>> {
    let BencodeValue::List(items) = value else {
        bail!("DHT values must be a list");
    };

    let mut peers = Vec::new();
    for item in items {
        peers.extend(parse_compact_peers(item.as_bytes()?)?);
    }
    Ok(peers)
}

fn parse_error(value: BencodeValue) -> Result<(i64, String)> {
    let BencodeValue::List(items) = value else {
        bail!("DHT error must be a list");
    };
    let code = items
        .first()
        .ok_or_else(|| anyhow!("invalid DHT error code"))?
        .as_int()?;
    let message = items
        .get(1)
        .ok_or_else(|| anyhow!("invalid DHT error message"))?
        .as_bytes()?;
    Ok((
        code,
        String::from_utf8(message.to_vec()).context("invalid DHT error message encoding")?,
    ))
}

#[derive(Debug, Clone, PartialEq)]
enum BencodeValue {
    Bytes(Vec<u8>),
    Int(i64),
    List(Vec<BencodeValue>),
    Dict(Vec<(Vec<u8>, BencodeValue)>),
}

impl BencodeValue {
    fn as_bytes(&self) -> Result<&[u8]> {
        match self {
            Self::Bytes(bytes) => Ok(bytes),
            _ => bail!("expected bencode bytes"),
        }
    }

    fn as_int(&self) -> Result<i64> {
        match self {
            Self::Int(value) => Ok(*value),
            _ => bail!("expected bencode integer"),
        }
    }
}

fn parse_bencode(data: &[u8]) -> Result<BencodeValue> {
    let (value, len) = parse_bencode_value(data)?;
    if len != data.len() {
        bail!("trailing bytes in bencode message");
    }
    Ok(value)
}

fn parse_bencode_value(data: &[u8]) -> Result<(BencodeValue, usize)> {
    if data.is_empty() {
        bail!("empty bencode input");
    }

    match data[0] {
        b'i' => {
            let end = data
                .iter()
                .position(|&byte| byte == b'e')
                .ok_or_else(|| anyhow!("unterminated bencode integer"))?;
            let digits = std::str::from_utf8(&data[1..end])
                .context("invalid bencode integer digits")?
                .parse::<i64>()
                .context("invalid bencode integer value")?;
            Ok((BencodeValue::Int(digits), end + 1))
        }
        b'l' => {
            let mut offset = 1;
            let mut items = Vec::new();
            while data[offset] != b'e' {
                let (item, len) = parse_bencode_value(&data[offset..])?;
                offset += len;
                items.push(item);
            }
            Ok((BencodeValue::List(items), offset + 1))
        }
        b'd' => {
            let mut offset = 1;
            let mut items = Vec::new();
            while data[offset] != b'e' {
                let (key, key_len) = parse_bencode_value(&data[offset..])?;
                let key = key.as_bytes()?.to_vec();
                offset += key_len;
                let (value, value_len) = parse_bencode_value(&data[offset..])?;
                offset += value_len;
                items.push((key, value));
            }
            Ok((BencodeValue::Dict(items), offset + 1))
        }
        b'0'..=b'9' => {
            let len = bencode_value_len(data)?;
            let colon = data
                .iter()
                .position(|&byte| byte == b':')
                .context("invalid bencode string")?;
            let str_len: usize = std::str::from_utf8(&data[..colon])
                .context("invalid bencode string length")?
                .parse()
                .context("invalid bencode string length value")?;
            let start = colon + 1;
            let end = start + str_len;
            Ok((
                BencodeValue::Bytes(data[start..end].to_vec()),
                len,
            ))
        }
        byte => bail!("unexpected bencode prefix: {byte}"),
    }
}

fn parse_compact_nodes(bytes: &[u8]) -> Result<Vec<DhtNode>> {
    if !bytes.len().is_multiple_of(26) {
        bail!("compact node info must be a multiple of 26 bytes");
    }

    let mut nodes = Vec::with_capacity(bytes.len() / 26);
    for chunk in bytes.chunks_exact(26) {
        let mut id = [0u8; 20];
        id.copy_from_slice(&chunk[..20]);
        let ip = Ipv4Addr::new(chunk[20], chunk[21], chunk[22], chunk[23]);
        let port = u16::from_be_bytes([chunk[24], chunk[25]]);
        nodes.push(DhtNode {
            id: NodeId(id),
            addr: SocketAddr::V4(SocketAddrV4::new(ip, port)),
        });
    }

    Ok(nodes)
}

fn parse_compact_peers(bytes: &[u8]) -> Result<Vec<SocketAddr>> {
    if !bytes.len().is_multiple_of(6) {
        bail!("compact peer info must be a multiple of 6 bytes");
    }

    let mut peers = Vec::with_capacity(bytes.len() / 6);
    for chunk in bytes.chunks_exact(6) {
        let ip = Ipv4Addr::new(chunk[0], chunk[1], chunk[2], chunk[3]);
        let port = u16::from_be_bytes([chunk[4], chunk[5]]);
        peers.push(SocketAddr::V4(SocketAddrV4::new(ip, port)));
    }

    Ok(peers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_compact_nodes_and_peers() {
        let nodes = parse_compact_nodes(&[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 127, 0, 0, 1,
            0x1a, 0xe1,
        ])
        .unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].addr, "127.0.0.1:6881".parse().unwrap());

        let peers = parse_compact_peers(&[127, 0, 0, 1, 0x1a, 0xe1]).unwrap();
        assert_eq!(peers, vec!["127.0.0.1:6881".parse().unwrap()]);
    }

    #[test]
    fn encodes_get_peers_query() {
        let tx = b"aa";
        let node_id = [0x11; 20];
        let info_hash = [0x22; 20];
        let packet = encode_query(tx, "get_peers", |args| {
            args.extend(encode_dict_pair(b"id", &encode_bytes(&node_id)));
            args.extend(encode_dict_pair(b"info_hash", &encode_bytes(&info_hash)));
            Ok(())
        })
        .unwrap();
        assert!(packet.starts_with(b"d"));
        assert!(packet.ends_with(b"e"));
        assert!(packet.windows(9).any(|window| window == b"get_peers"));
    }

    #[test]
    #[ignore = "requires network access to the public DHT"]
    fn finds_arch_linux_peers_on_live_dht() {
        let info_hash: [u8; 20] = [
            0x77, 0x76, 0x95, 0x04, 0x96, 0x23, 0xa1, 0xcd, 0x05, 0x2b, 0xd6, 0xb1, 0x75, 0xb4,
            0x0e, 0x65, 0x40, 0xce, 0x74, 0xca,
        ];
        let peers = find_peers(info_hash).unwrap();
        assert!(!peers.is_empty(), "expected peers for Arch Linux ISO");
    }
}
