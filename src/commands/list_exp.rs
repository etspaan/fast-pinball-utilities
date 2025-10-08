use crate::fast_monitor::{ExpBoardInfo, FastPinballMonitor};

pub fn run(fpm: &mut FastPinballMonitor) {
    let boards: Vec<ExpBoardInfo> = fpm.list_connected_exp_boards();
    if boards.is_empty() {
        println!("No EXP boards found.");
    } else {
        println!("EXP boards:");
        for b in boards {
            println!(
                "  Address {} -> {} (version {})",
                b.address, b.board_name, b.version
            );
        }
    }
}
