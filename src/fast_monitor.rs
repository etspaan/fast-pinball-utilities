use crate::protocol::exp_protocol::ExpProtocol;
use crate::protocol::net_protocol::NetProtocol;
use serialport::{DataBits, FlowControl, Parity, StopBits, available_ports};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Protocol {
    NET,
    EXP,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpBoardInfo {
    pub address: String,
    pub board_name: String,
    pub version: String,
    pub available_versions: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetBoardInfo {
    pub node_id: String,
    pub node_name: String,
    pub firmware: String,
    // All additional numeric/config fields returned after the firmware version, in order
    pub extra_fields: Vec<String>,
}

pub struct FastPinballMonitor {
    pub net: NetProtocol,
    pub exp: ExpProtocol,
}
impl FastPinballMonitor {
    pub fn connect() -> Option<Self> {
        let ids = Self::discover_protocol_ports();

        let mut net_opt: Option<NetProtocol> = None;
        let mut exp_opt: Option<ExpProtocol> = None;
        for (port, proto) in ids.iter() {
            match proto {
                Protocol::NET => {
                    if net_opt.is_none() {
                        net_opt = Some(NetProtocol::new(port.clone()));
                    }
                }
                Protocol::EXP => {
                    if exp_opt.is_none() {
                        exp_opt = Some(ExpProtocol::new(port.clone()));
                    }
                }
            }
        }

        match (net_opt, exp_opt) {
            (Some(net), Some(exp)) => Some(FastPinballMonitor { net, exp }),
            _ => None,
        }
    }

    pub fn list_connected_exp_boards(&mut self) -> Vec<ExpBoardInfo> {
        let mut results: Vec<ExpBoardInfo> = Vec::new();

        // Small helper to drain any pending bytes before we start
        let _ = self.exp.receive();

        // Use the centralized EXP address mapping constant and the static firmware map
        use crate::constants::{AVAILABLE_FIRMWARE_VERSIONS, EXP_ADDRESS_MAP};

        // Iterate addresses, send ID@{Address}: and collect parsed responses
        for &(addr, board_type) in EXP_ADDRESS_MAP.iter() {
            let cmd = format!("ID@{}:\r", addr);

            self.exp.send(cmd.into_bytes());
            std::thread::sleep(Duration::from_millis(10));

            let resp = self.exp.receive();

            if let Some((proto, board, version)) = parse_id_response(&resp) {
                let board_name = if board.is_empty() {
                    board_type.to_string()
                } else {
                    board
                };
                let key = format!("{}_{}", board_name, proto);
                let fallback_key = format!("{}_{}", board_type, proto);
                // Translate the available firmware map (version -> path) into a list of versions
                let versions_from_map = |m: &HashMap<String, HashMap<String, String>>,
                                         k: &str|
                 -> Option<Vec<String>> {
                    m.get(k).map(|inner| {
                        let mut v: Vec<String> = inner.keys().cloned().collect();
                        v.sort();
                        v
                    })
                };
                let available_versions = versions_from_map(&AVAILABLE_FIRMWARE_VERSIONS, &key)
                    .or_else(|| versions_from_map(&AVAILABLE_FIRMWARE_VERSIONS, &fallback_key));
                results.push(ExpBoardInfo {
                    address: addr.to_string(),
                    board_name,
                    version,
                    available_versions,
                });
            }

            // Small delay between polls to be gentle on the bus
            std::thread::sleep(Duration::from_millis(5));
        }

        results
    }

    pub fn list_connected_net_boards(&mut self) -> HashMap<usize, NetBoardInfo> {
        let mut results: HashMap<usize, NetBoardInfo> = HashMap::new();

        // Drain any pending bytes from NET before starting
        let _ = self.net.receive();

        // Also query the Neuron controller directly via ID:\r to get its own info
        let controller_info: Option<(String, String)> = {
            let _ = self.net.send(b"ID:\r");
            std::thread::sleep(Duration::from_millis(10));
            let resp = self.net.receive();
            if let Some((_proto, board, version)) = parse_id_response(&resp) {
                Some((board, version))
            } else {
                None
            }
        };

        let mut index: usize = 0;
        loop {
            let node_id_str = format!("{:02}", index);
            let cmd = format!("NN:{}\r", node_id_str);
            let _ = self.net.send(cmd.as_bytes());
            std::thread::sleep(Duration::from_millis(10));

            let resp = self.net.receive();
            if resp.is_empty() || resp.contains("!Node Not Found!") {
                // No response or node not found: stop scanning
                break;
            }

            if let Some(info) = parse_nn_response(&resp) {
                results.insert(index, info);
            }

            index += 1;
            // Be gentle on the bus
            std::thread::sleep(Duration::from_millis(5));
        }

        // Add the Neuron controller (from ID:) as its own entry, without overriding NN data
        if let Some((board, version)) = controller_info.clone() {
            let neuron_info = NetBoardInfo {
                node_id: "NC".to_string(),
                node_name: board,
                firmware: version,
                extra_fields: Vec::new(),
            };
            // Use the next available index so we don't collide with NN-reported nodes
            results.insert(index, neuron_info);
        }

        results
    }

    fn discover_protocol_ports() -> HashMap<String, Protocol> {
        let mut results: HashMap<String, Protocol> = HashMap::new();
        match available_ports() {
            Ok(ports) => {
                for port in ports {
                    if let Ok(mut serial_port) = serialport::new(port.port_name.clone(), 921_600)
                        .data_bits(DataBits::Eight)
                        .parity(Parity::None)
                        .stop_bits(StopBits::One)
                        .dtr_on_open(true)
                        .flow_control(FlowControl::None)
                        .timeout(Duration::from_millis(5))
                        .open()
                    {
                        // Try to identify the device by sending the ID command
                        let _ = serial_port.write_all(b"ID:\r");
                        // Give the device a moment to respond
                        std::thread::sleep(Duration::from_millis(5));

                        let mut buf_bytes = [0u8; 256];
                        let mut collected = Vec::new();
                        loop {
                            match serial_port.read(&mut buf_bytes) {
                                Ok(0) => break,
                                Ok(n) => {
                                    collected.extend_from_slice(&buf_bytes[..n]);
                                    if collected.len() >= 256 {
                                        break;
                                    }
                                }
                                Err(e) => {
                                    let kind = e.kind();
                                    if kind == std::io::ErrorKind::WouldBlock
                                        || kind == std::io::ErrorKind::TimedOut
                                    {
                                        break;
                                    } else {
                                        break;
                                    }
                                }
                            }
                        }
                        if !collected.is_empty() {
                            let s = String::from_utf8_lossy(&collected).trim().to_string();
                            if let Some(proto) = parse_protocol(&s) {
                                results.insert(port.port_name.clone(), proto);
                            }
                        }
                    }
                }
            }
            Err(_) => {}
        }
        results
    }
}

fn parse_protocol(resp: &str) -> Option<Protocol> {
    // Look for "ID:" and parse the following alpha token (e.g., NET or EXP)
    let after = resp.split_once("ID:")?.1;
    let token = after
        .trim()
        .split(|c: char| !c.is_ascii_alphabetic())
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    match token.as_str() {
        "NET" => Some(Protocol::NET),
        "EXP" => Some(Protocol::EXP),
        _ => None,
    }
}

fn parse_id_response(resp: &str) -> Option<(String, String, String)> {
    // Expected formats:
    // "ID:{Protocol} {BoardName} {Version}"
    // Be tolerant of commas after the protocol token (e.g., "ID:EXP, FP-EXP-0091 v0.48")
    let after = resp.split_once("ID:")?.1;
    // Normalize commas to spaces and trim
    let normalized = after.replace(',', " ");
    let mut parts = normalized.split_whitespace();
    let protocol = parts.next()?.to_string();
    let board = parts.next()?.to_string();
    let version = parts.next()?.to_string();
    Some((protocol, board, version))
}

fn parse_nn_response(resp: &str) -> Option<NetBoardInfo> {
    // Find the last occurrence of an NN: response within the buffer
    let idx = resp.rfind("NN:")?;
    let after = &resp[idx + 3..];

    // Take until end of line or whole remainder
    let line = after.lines().next().unwrap_or(after).trim();

    // Split by commas into fields
    let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
    if parts.len() < 3 {
        return None;
    }

    let node_id = parts[0].to_string();
    let node_name = parts[1].to_string();
    let firmware = parts[2].to_string();
    let extra_fields = if parts.len() > 3 {
        parts[3..].iter().map(|s| s.to_string()).collect()
    } else {
        Vec::new()
    };

    Some(NetBoardInfo {
        node_id,
        node_name,
        firmware,
        extra_fields,
    })
}
