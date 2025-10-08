use std::io::{self};

pub fn read_line_trimmed() -> String {
    let mut s = String::new();
    let _ = io::stdin().read_line(&mut s);
    s.trim().to_string()
}
