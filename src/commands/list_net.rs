use crate::fast_monitor::{FastPinballMonitor, NetBoardInfo};
use std::collections::BTreeMap;

pub fn run(fpm: &mut FastPinballMonitor) {
    let boards = fpm.list_connected_net_boards();
    if boards.is_empty() {
        println!("No NET boards found.");
    } else {
        println!("NET nodes:");
        // Ensure stable ordered output by node id
        let mut ordered: BTreeMap<usize, NetBoardInfo> = BTreeMap::new();
        for (k, v) in boards.into_iter() {
            ordered.insert(k, v);
        }
        for (_k, NetBoardInfo { node_id, node_name, firmware, .. }) in ordered.into_iter() {
            println!("  Node {} ({}) -> firmware {}", node_id, node_name, firmware);
        }
    }
}
