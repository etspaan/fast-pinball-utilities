use std::io::{self, Write};
use crate::constants::AVAILABLE_FIRMWARE_VERSIONS;
use crate::fast_monitor::FastPinballMonitor;
use crate::commands::utils::read_line_trimmed;

pub fn run(fpm: &mut FastPinballMonitor) {
    let key = "FP-CPU-2000_NET";
    let maybe = AVAILABLE_FIRMWARE_VERSIONS.get(key);
    let mut versions: Vec<String> = match maybe {
        Some(map) => map.keys().cloned().collect(),
        None => Vec::new(),
    };
    if versions.is_empty() {
        println!(
            "No NET firmware files found. Place files under src\\firmware\\FP-CPU-2000 and try again."
        );
        return;
    }
    versions.sort();
    versions.reverse();
    println!("Available NET firmware versions (newest first):");
    for (i, v) in versions.iter().enumerate() {
        println!("  {}) {}", i + 1, v);
    }
    print!(
        "Enter version number (1-{}), or 0 to cancel: ",
        versions.len()
    );
    let _ = io::stdout().flush();
    let sel = read_line_trimmed();
    let Ok(mut idx) = sel.parse::<usize>() else {
        println!("Invalid selection.");
        return;
    };
    if idx == 0 {
        println!("Canceled.");
        return;
    }
    if idx < 1 || idx > versions.len() {
        println!("Out of range.");
        return;
    }
    idx -= 1;
    let version = versions[idx].clone();

    println!("About to flash NET (CPU) to version {}.", version);
    print!("Proceed? [y/N]: ");
    let _ = io::stdout().flush();
    let confirm = read_line_trimmed();
    if !matches!(confirm.as_str(), "y" | "Y" | "yes" | "YES") {
        println!("Canceled.");
        return;
    }

    println!("Starting NET firmware update... This may take a few minutes.");
    fpm.net.update_firmware(&version);
}
