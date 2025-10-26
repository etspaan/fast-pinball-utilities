use std::env;
use crate::fast_monitor::FastPinballMonitor;

pub mod constants;
pub mod fast_monitor;
pub mod protocol;
pub mod commands;

fn print_help(program: &str) {
    println!("{} - FAST Pinball utility", program);
    println!("Usage:");
    println!(
        "  {} list-exp       List connected EXP boards and their versions",
        program
    );
    println!(
        "  {} list-net       List connected NET boards and their versions",
        program
    );
    println!(
        "  {} list           List both EXP and NET boards (default)",
        program
    );
    println!(
        "  {} update-exp     Interactive mode to select an EXP board and flash a chosen version",
        program
    );
    println!(
        "  {} update-net     Interactive mode to flash the NET (CPU) firmware",
        program
    );
    println!(
        "  {} get-latest-firmware  Download latest firmware files into ~/.fast/firmware",
        program
    );
    println!("  {} help           Show this help", program);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args.get(0).map(|s| s.as_str()).unwrap_or("fast-util");

    let mode = if args.len() <= 1 {
        "list".to_string()
    } else {
        args[1].to_ascii_lowercase()
    };

    match mode.as_str() {
        "help" | "-h" | "--help" => {
            print_help(program);
            return;
        }
        _ => {}
    }

    // Handle check-for-updates without requiring hardware
    if matches!(
        mode.as_str(),
        "get-latest-firmware" | "check-updates" | "download-firmware" | "check"
    ) {
        match commands::run_check_updates() {
            Ok(_) => std::process::exit(0),
            Err(e) => {
                eprintln!("Failed to download firmware: {}", e);
                std::process::exit(1);
            }
        }
    }

    let fpm = FastPinballMonitor::connect();
    let mut fpm = match fpm {
        Some(fpm) => fpm,
        None => {
            eprintln!(
                "Could not find FAST NET/EXP serial ports. Ensure devices are connected and accessible."
            );
            std::process::exit(2);
        }
    };

    match mode.as_str() {
        "update-exp" | "update" | "flash" => {
            commands::run_update_exp(&mut fpm);
        }
        "update-net" | "flash-net" | "net-update" => {
            commands::run_update_net(&mut fpm);
        }
        "list-exp" | "exp" => {
            commands::run_list_exp(&mut fpm);
        }
        "list-net" | "net" => {
            commands::run_list_net(&mut fpm);
        }
        "list" | "all" | _ => {
            commands::run_list_exp(&mut fpm);
            println!();
            commands::run_list_net(&mut fpm);
        }
    }
}
