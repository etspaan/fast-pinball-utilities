use indicatif::{ProgressBar, ProgressStyle};
use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};
use std::io::{BufReader, Read, Write};
use std::time::Duration;

pub struct ExpProtocol {
    pub serial_port: Box<dyn SerialPort>,
}

impl ExpProtocol {
    pub fn new(port: String) -> Self {
        let serial_port = serialport::new(port, 921_600)
            .data_bits(DataBits::Eight)
            .parity(Parity::None)
            .stop_bits(StopBits::One)
            .dtr_on_open(true)
            .flow_control(FlowControl::None)
            .timeout(Duration::from_millis(5))
            .open()
            .unwrap();

        Self { serial_port }
    }

    /// Update EXP board firmware by board address and version.
    ///
    /// Looks up the board type using EXP_ADDRESS_MAP and resolves the firmware
    /// file path from AVAILABLE_FIRMWARE_VERSIONS using key `{BoardType}_EXP`
    /// and the provided version (normalized as `major.minor` with a two-digit
    /// minor, e.g., `1.05`). Streams the file to the serial port.
    pub fn update_firmware(&mut self, address_hex: &str, version: &str) {
        use crate::constants::{AVAILABLE_FIRMWARE_VERSIONS, EXP_ADDRESS_MAP};

        // Find the board type by address (case-insensitive match on hex string)
        let addr_upper = address_hex.to_ascii_uppercase();
        let board_type = EXP_ADDRESS_MAP
            .iter()
            .find(|(addr, _)| addr.to_ascii_uppercase() == addr_upper)
            .map(|(_, bt)| *bt);

        if board_type.is_none() {
            eprintln!("Unknown EXP board address: {}", address_hex);
            return;
        }
        let board_type = board_type.unwrap();

        // Normalize version to the stored format (e.g., 1.5 -> 1.05)
        let normalized_version = {
            let mut out = version.to_string();
            if let Some((maj_s, min_s)) = version.split_once('.') {
                if let (Ok(maj), Ok(min)) = (maj_s.parse::<u32>(), min_s.parse::<u32>()) {
                    out = format!("{}.{}", maj, format!("{:02}", min));
                }
            }
            out
        };

        // Build key and resolve file path
        let key = format!("{}_{}", board_type, "EXP");
        let file_path_opt = AVAILABLE_FIRMWARE_VERSIONS
            .get(&key)
            .and_then(|inner| inner.get(&normalized_version))
            .cloned();

        let Some(file_path) = file_path_opt else {
            eprintln!(
                "Firmware not found for key '{}' version '{}'. Available: {:?}",
                key,
                normalized_version,
                AVAILABLE_FIRMWARE_VERSIONS
                    .get(&key)
                    .map(|m| m.keys().cloned().collect::<Vec<_>>())
            );
            return;
        };

        // Target the correct board address with the EXP Address command (lowercase per spec example)
        self.send(format!("ea:{}\r", address_hex).into_bytes());
        std::thread::sleep(Duration::from_millis(10));
        // Optionally read any immediate response/echo to clear buffer
        let _ = self.receive();

        // Open file and stream line by line (as bytes), preserving existing line endings (CRLF)
        // Display progress using indicatif
        let total_size = match std::fs::metadata(&file_path) {
            Ok(m) => m.len(),
            Err(_) => 0,
        };

        let pb = if total_size > 0 {
            let pb = ProgressBar::new(total_size);
            let style = ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta}) - {msg}")
                .unwrap()
                .progress_chars("##-");
            pb.set_style(style);
            pb.set_message(format!("Flashing {}", file_path));
            pb
        } else {
            let pb = ProgressBar::new_spinner();
            pb.enable_steady_tick(Duration::from_millis(100));
            pb.set_message(format!("Flashing {} (size unknown)", file_path));
            let style = ProgressStyle::with_template(
                "{spinner:.green} {elapsed_precise} {bytes} sent - {msg}",
            )
            .unwrap();
            pb.set_style(style);
            pb
        };

        match std::fs::File::open(&file_path) {
            Ok(file) => {
                use std::io::BufRead;
                let mut reader = BufReader::new(file);
                let mut line: Vec<u8> = Vec::with_capacity(1024);
                let mut bytes_sent: u64 = 0;
                loop {
                    line.clear();
                    match reader.read_until(b'\r', &mut line) {
                        Ok(0) => break, // EOF
                        Ok(_n) => {
                            let _ = self.serial_port.write_all(&line);
                            let _ = self.serial_port.flush();

                            // Update progress bar
                            bytes_sent = bytes_sent.saturating_add(line.len() as u64);
                            if total_size > 0 {
                                pb.set_position(bytes_sent.min(total_size));
                            } else {
                                pb.set_message(format!(
                                    "Flashing {} ({} bytes sent)",
                                    file_path, bytes_sent
                                ));
                            }

                            // Small delay between chunks
                            std::thread::sleep(Duration::from_millis(200));
                        }
                        Err(e) => {
                            eprintln!("Failed while reading firmware file '{}': {}", file_path, e,);
                            break;
                        }
                    }
                }

                // Finish the progress bar
                if total_size > 0 {
                    pb.finish_with_message("Done");
                } else {
                    pb.finish_and_clear();
                }
            }
            Err(e) => {
                pb.finish_and_clear();
                eprintln!("Failed to open firmware file '{}': {}", file_path, e,);
            }
        }

        // Wait for bootloader completion acknowledgment "!BL2040:02"
        let mut accumulate = String::new();
        let start_wait = std::time::Instant::now();
        let boot_timeout = Duration::from_secs(30);
        let mut saw_boot_ok = false;
        while start_wait.elapsed() < boot_timeout {
            let resp = self.receive();
            if !resp.is_empty() {
                accumulate.push_str(&resp);
                // Print any intermediate output to aid debugging
                // println!("[RX] {}", resp);
                if accumulate.contains("!BL2040:02") {
                    saw_boot_ok = true;
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        if !saw_boot_ok {
            eprintln!(
                "Timed out waiting for bootloader completion (!BL2040:02). Proceeding to ID check anyway..."
            );
        } else {
            println!("Bootloader reported completion: !BL2040:02");
        }

        // Query the device ID and firmware version for the target address
        let id_cmd = format!("ID@{}:\r", address_hex);
        self.send(id_cmd.into_bytes());

        // Collect ID response for up to 5 seconds
        let verify_timeout = Duration::from_secs(5);
        let start_verify = std::time::Instant::now();
        let mut id_resp = String::new();
        while start_verify.elapsed() < verify_timeout {
            let r = self.receive();
            if !r.is_empty() {
                id_resp.push_str(&r);
            }
            // If the device echoes or provides line breaks, we may get the full response early
            if id_resp.len() > 0 {
                // simple heuristic
                // try to break early if we already have a newline or colon-rich response
                if id_resp.contains('\n') || id_resp.contains('\r') {
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        println!("ID response: {}", id_resp);

        // Parse and validate the expected ID response format: "ID:EXP {BoardName} {version}"
        let expected_board = board_type;
        let expected_ver = normalized_version;
        let mut found_line = None::<String>;
        let mut parsed_board = None::<String>;
        let mut parsed_version = None::<String>;
        let mut verified = false;

        for line in id_resp.lines() {
            let l = line.trim();
            if l.starts_with("ID:EXP") {
                found_line = Some(l.to_string());
                // Tokenize by whitespace; expected tokens: ["ID:EXP", "{BoardName}", "{version}"]
                let parts: Vec<&str> = l.split_whitespace().collect();
                if parts.len() >= 3 {
                    parsed_board = Some(parts[1].to_string());
                    // Extract version token and strip any trailing non-digit/dot characters
                    let mut ver = parts[2].trim().to_string();
                    while ver.ends_with(|c: char| !c.is_ascii_digit() && c != '.') {
                        ver.pop();
                    }
                    parsed_version = Some(ver.clone());
                    if parts[1] == expected_board && ver == expected_ver {
                        verified = true;
                        break;
                    }
                }
            }
        }

        if verified {
            println!(
                "Firmware update verified: board {} reports version {} at address {}",
                expected_board, expected_ver, address_hex
            );
        } else {
            // Provide helpful diagnostics
            if let (Some(pb), Some(pv)) = (parsed_board.as_deref(), parsed_version.as_deref()) {
                if pb != expected_board {
                    eprintln!(
                        "Warning: ID board mismatch. Expected '{}', got '{}' (line: {:?}).",
                        expected_board, pb, found_line
                    );
                }
                if pv != expected_ver {
                    eprintln!(
                        "Warning: Firmware version mismatch. Expected '{}', got '{}' (line: {:?}).",
                        expected_ver, pv, found_line
                    );
                }
            } else if let Some(line) = found_line {
                eprintln!(
                    "Warning: Could not parse board/version from ID line: {:?}. Expected format: 'ID:EXP {{BoardName}} {{version}}'",
                    line
                );
            } else {
                eprintln!(
                    "Warning: No 'ID:EXP' line found in response; cannot verify flashed version {} for board {}.",
                    expected_ver, expected_board
                );
            }
        }
    }

    pub fn send(&mut self, command: Vec<u8>) {
        // Best-effort write; avoid panicking on errors
        let _ = self.serial_port.write_all(command.as_slice());
        let _ = self.serial_port.flush();
    }

    pub fn receive(&mut self) -> String {
        let mut buf_bytes = [0u8; 256];
        let mut collected = Vec::new();

        match self.serial_port.read(&mut buf_bytes) {
            Ok(0) => {}
            Ok(n) => {
                collected.extend_from_slice(&buf_bytes[..n]);
                if collected.len() >= 256 {}
            }
            Err(_) => {}
        }

        String::from_utf8_lossy(&collected).trim().to_string()
    }
}
