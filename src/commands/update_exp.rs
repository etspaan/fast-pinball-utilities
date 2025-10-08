use std::io::{self, Write};
use crate::fast_monitor::{ExpBoardInfo, FastPinballMonitor};
use crate::commands::utils::read_line_trimmed;

pub fn run(fpm: &mut FastPinballMonitor) {
    // List EXP boards and let the user choose one
    let boards: Vec<ExpBoardInfo> = fpm.list_connected_exp_boards();
    if boards.is_empty() {
        println!("No EXP boards found. Connect a board and try again.");
        return;
    }
    println!("Select an EXP board to flash:");
    for (i, b) in boards.iter().enumerate() {
        println!(
            "  {}) Address {} -> {} (current {})",
            i + 1,
            b.address,
            b.board_name,
            b.version
        );
    }
    print!("Enter number (1-{}), or 0 to cancel: ", boards.len());
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
    if idx < 1 || idx > boards.len() {
        println!("Out of range.");
        return;
    }
    idx -= 1;

    // Extract chosen board info (owned strings)
    let chosen = &boards[idx];
    let address = chosen.address.clone();
    let board_name = chosen.board_name.clone();
    let current_version = chosen.version.clone();
    let mut versions: Vec<String> = chosen
        .available_versions
        .clone()
        .unwrap_or_else(|| Vec::new());

    if versions.is_empty() {
        println!(
            "No firmware files available for {}. Place firmware files in src\\firmware and try again.",
            board_name
        );
        return;
    }
    // Sort descending so newest (highest) appears first
    versions.sort();
    versions.reverse();

    println!(
        "Available versions for {} (current {}):",
        board_name, current_version
    );
    for (i, v) in versions.iter().enumerate() {
        println!(
            "  {}) {}{}",
            i + 1,
            v,
            if *v == current_version {
                "  (installed)"
            } else {
                ""
            }
        );
    }
    print!(
        "Enter version number (1-{}), or 0 to cancel: ",
        versions.len()
    );
    let _ = io::stdout().flush();
    let vsel = read_line_trimmed();
    let Ok(mut vidx) = vsel.parse::<usize>() else {
        println!("Invalid selection.");
        return;
    };
    if vidx == 0 {
        println!("Canceled.");
        return;
    }
    if vidx < 1 || vidx > versions.len() {
        println!("Out of range.");
        return;
    }
    vidx -= 1;
    let version = versions[vidx].clone();

    println!(
        "About to flash {} at address {} to version {}.",
        board_name, address, version
    );
    print!("Proceed? [y/N]: ");
    let _ = io::stdout().flush();
    let confirm = read_line_trimmed();
    if !matches!(confirm.as_str(), "y" | "Y" | "yes" | "YES") {
        println!("Canceled.");
        return;
    }

    // Perform update
    println!("Starting firmware update... This may take a few minutes.");
    fpm.exp.update_firmware(&address, &version);
}
