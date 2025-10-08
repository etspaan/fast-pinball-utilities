# FAST Pinball Utilities

A small Rust command‑line tool to help work with FAST Pinball controller hardware via the serial ports exposed by the NET (CPU) and EXP (I/O) boards. It can:

- Discover and list connected EXP and NET boards with their reported firmware versions
- Interactively flash EXP node boards
- Interactively flash the NET (CPU) firmware
- Download the latest firmware text files from the official fastpinball/fast-firmware repository into a local cache

The goal is to make common maintenance tasks (checking what is attached and updating firmware) quick and repeatable from a single CLI.

## Requirements

- Rust toolchain (to build from source): https://www.rust-lang.org/tools/install
- Access to serial ports where your FAST NET/EXP hardware is attached
  - On first run, ensure you have OS permissions to open serial ports (e.g., add your user to the dialout group on Linux, run from an elevated terminal on Windows if needed)
- Internet connectivity for the optional firmware download command

## Build and Install

Build locally:

- Debug build: `cargo build`
- Release build: `cargo build --release`

Install to your Cargo bin directory:

- `cargo install --path .`

After installing, the binary will be available as `fast-pinball-utilities` (or you can run with `cargo run -- <command>` during development).

## Usage

Run the program without arguments to list both EXP and NET boards:

- `fast-pinball-utilities`

Show help and all commands:

- `fast-pinball-utilities help`

Available commands (aliases in parentheses):

- `list-exp` (`exp`) — list connected EXP boards and their versions
- `list-net` (`net`) — list connected NET boards and their versions
- `list` (`all`) — list both EXP and NET boards (default behavior)
- `update-exp` (`update`, `flash`) — interactive flow to select an EXP board and flash a chosen firmware version
- `update-net` (`flash-net`, `net-update`) — interactive flow to flash the NET (CPU) firmware
- `get-latest-firmware` (`check-updates`, `download-firmware`, `check`) — download the latest firmware text files to a local cache

### Firmware download location

When you run `get-latest-firmware`, firmware files are downloaded from:

- https://github.com/fastpinball/fast-firmware (main branch ZIP)

They are extracted to:

- `~/.fast/firmware` (your home directory under a `.fast/firmware` folder)

Only `.txt` firmware files from the archive are stored, keeping the directory compact and ready for use by the flashing commands.

### Flashing notes

- The tool communicates over the same serial ports you would use manually. It discovers and opens the detected NET and EXP ports automatically.
- Flashing is interactive: you pick the target board and the version to apply from the files available in `~/.fast/firmware`.
- The NET update also attempts to update remaining node boards afterward when appropriate.
- After flashing, the tool tries to verify the reported firmware version and will print warnings if the device’s ID does not match the expected board/version.

### Examples

- List everything:
  - `fast-pinball-utilities`
- Just EXP boards:
  - `fast-pinball-utilities list-exp`
- Just NET boards:
  - `fast-pinball-utilities list-net`
- Download latest firmware files:
  - `fast-pinball-utilities get-latest-firmware`
- Flash an EXP board (interactive):
  - `fast-pinball-utilities update-exp`
- Flash the NET (CPU) firmware (interactive):
  - `fast-pinball-utilities update-net`

## Troubleshooting

- "Could not find FAST NET/EXP serial ports": Ensure hardware is connected and recognized by your OS. Verify the correct drivers are installed and that your user has permission to access serial devices.
- If flashing appears to stall, check cabling and power. You may also try reconnecting the device and re-running the command.
- If verification prints a mismatch warning, confirm you selected the correct firmware file for the board you’re updating.

## Project

- Package name: `fast-pinball-utilities`
- Edition: Rust 2024

This repository is intended for developers and technicians working with FAST Pinball hardware who prefer or require a command‑line workflow for inventory and firmware maintenance.
