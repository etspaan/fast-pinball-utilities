use indicatif::{ProgressBar, ProgressStyle};
use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};
use std::io::Read;
use std::time::Duration;

pub struct NetProtocol {
    pub serial_port: Box<dyn SerialPort>,
}

impl NetProtocol {
    pub fn new(port: String) -> Self {
        let serial_port = serialport::new(port, 921_600)
            .data_bits(DataBits::Eight)
            .flow_control(FlowControl::None)
            .stop_bits(StopBits::One)
            .parity(Parity::None)
            .dtr_on_open(true)
            .timeout(Duration::from_millis(200))
            .open()
            .unwrap();

        Self { serial_port }
    }

    /// Update NET (CPU) firmware by version string (e.g., "2.28" or "2.8").
    ///
    /// Looks up the firmware file using the key "FP-CPU-2000_NET" within
    /// AVAILABLE_FIRMWARE_VERSIONS, streams it to the NET port, waits for the
    /// bootloader completion token, then verifies via ID. No address is required.
    pub fn update_firmware(&mut self, version: &str) {
        use crate::constants::AVAILABLE_FIRMWARE_VERSIONS;

        // Normalize version to the stored format (e.g., 2.8 -> 2.08)
        let normalized_version = {
            let mut out = version.to_string();
            if let Some((maj_s, min_s)) = version.split_once('.') {
                if let (Ok(maj), Ok(min)) = (maj_s.parse::<u32>(), min_s.parse::<u32>()) {
                    out = format!("{}.{}", maj, format!("{:02}", min));
                }
            }
            out
        };

        let key = "FP-CPU-2000_NET".to_string();
        let file_path_opt = AVAILABLE_FIRMWARE_VERSIONS
            .get(&key)
            .and_then(|inner| inner.get(&normalized_version))
            .cloned();

        let Some(file_path) = file_path_opt else {
            eprintln!(
                "NET firmware not found for version '{}'. Available: {:?}",
                normalized_version,
                AVAILABLE_FIRMWARE_VERSIONS
                    .get(&key)
                    .map(|m| m.keys().cloned().collect::<Vec<_>>())
            );
            return;
        };

        // Drain any pending input
        let _ = self.receive();

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
                let mut reader = std::io::BufReader::new(file);
                let mut line: Vec<u8> = Vec::with_capacity(1024);
                let mut bytes_sent: u64 = 0;
                loop {
                    line.clear();
                    match reader.read_until(b'\r', &mut line) {
                        Ok(0) => break, // EOF
                        Ok(_) => {
                            let _ = self.serial_port.write_all(&line);
                            let _ = self.serial_port.flush();

                            bytes_sent = bytes_sent.saturating_add(line.len() as u64);
                            if total_size > 0 {
                                pb.set_position(bytes_sent.min(total_size));
                            } else {
                                pb.set_message(format!(
                                    "Flashing {} ({} bytes sent)",
                                    file_path, bytes_sent
                                ));
                            }

                            std::thread::sleep(Duration::from_millis(400));
                        }
                        Err(e) => {
                            eprintln!(
                                "Failed while reading NET firmware file '{}': {}",
                                file_path, e
                            );
                            break;
                        }
                    }
                }

                if total_size > 0 {
                    pb.finish_with_message("Done");
                } else {
                    pb.finish_and_clear();
                }
            }
            Err(e) => {
                pb.finish_and_clear();
                eprintln!("Failed to open NET firmware file '{}': {}", file_path, e);
                return;
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
                if accumulate.contains("!B:02") {
                    saw_boot_ok = true;
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        if !saw_boot_ok {
            eprintln!(
                "Timed out waiting for bootloader completion (!B:02). Proceeding to ID check..."
            );
        } else {
            println!("Bootloader reported completion: !B:02");
        }

        // Query the device ID and firmware version for NET
        let _ = self.send(b"ID:\r");

        // Collect ID response for up to 5 seconds
        let verify_timeout = Duration::from_secs(5);
        let start_verify = std::time::Instant::now();
        let mut id_resp = String::new();
        while start_verify.elapsed() < verify_timeout {
            let r = self.receive();
            if !r.is_empty() {
                id_resp.push_str(&r);
            }
            if id_resp.contains('\n') || id_resp.contains('\r') {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        println!("ID response: {}", id_resp);

        // Parse and validate the expected ID response format: "ID:NET {BoardName} {version}"
        let expected_board = "FP-CPU-2000".to_string();
        let expected_ver = normalized_version;
        let mut found_line = None::<String>;
        let mut parsed_board = None::<String>;
        let mut parsed_version = None::<String>;
        let mut verified = false;
        for line in id_resp.lines() {
            let l = line.trim();
            if l.starts_with("ID:NET") {
                found_line = Some(l.to_string());
                let parts: Vec<&str> = l.split_whitespace().collect();
                if parts.len() >= 3 {
                    parsed_board = Some(parts[1].to_string());
                    let mut ver = parts[2].trim().to_string();
                    // Remove any trailing non-digit/dot characters (e.g., CR/LF or annotations)
                    while ver.ends_with(|c: char| !c.is_ascii_digit() && c != '.') {
                        ver.pop();
                    }
                    // Trim leading zeros from the major portion (e.g., "02.28" -> "2.28")
                    let ver = if let Some((maj, rest)) = ver.split_once('.') {
                        let maj_trim = maj.trim_start_matches('0');
                        let maj_norm = if maj_trim.is_empty() { "0" } else { maj_trim };
                        format!("{}.{}", maj_norm, rest)
                    } else {
                        // No dot present; just trim leading zeros of the whole string
                        let trimmed = ver.trim_start_matches('0');
                        if trimmed.is_empty() { "0".to_string() } else { trimmed.to_string() }
                    };

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
                "NET firmware update verified: board {} reports version {}",
                expected_board, expected_ver
            );
        } else {
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
                    "Warning: Could not parse board/version from ID line: {:?}. Expected format: 'ID:NET {{BoardName}} {{version}}'",
                    line
                );
            } else {
                eprintln!(
                    "Warning: No 'ID:NET' line found in response; cannot verify flashed version {} for board {}.",
                    expected_ver, expected_board
                );
            }
        }

        println!("Attempting to update remaining node boards. Not all I/O boards may have an update.");
        // Update the remaining node boards
        _ =self.send(b"bn:aa55\r");



    }

    pub fn send(&mut self, command: &[u8]) -> std::io::Result<()> {
        use std::io::{ErrorKind, Write};
        // Retry on Interrupted, propagate other errors
        loop {
            match self.serial_port.write_all(command) {
                Ok(()) => {
                    // Best-effort flush; ignore WouldBlock and other flush errors
                    let _ = self.serial_port.flush();
                    return Ok(());
                }
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
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
            Err(_e) => {}
        }

        String::from_utf8_lossy(&collected).trim().to_string()
    }
}
