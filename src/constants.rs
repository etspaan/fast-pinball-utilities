// Centralized constants for the project.
// EXP board address-to-type mapping from FAST documentation.
// Each entry is (address_hex, board_type)

use once_cell::sync::Lazy;
use std::collections::HashMap;

pub const EXP_ADDRESS_MAP: [(&str, &str); 25] = [
    ("48", "FP-CPU-2000"), // Neuron built-in EXP (address 48)
    ("D0", "FP-EXP-0051"), // FP-EXP-0051 (D0-D3)
    ("D1", "FP-EXP-0051"),
    ("D2", "FP-EXP-0051"),
    ("D3", "FP-EXP-0051"),
    ("90", "FP-EXP-0061"), // FP-EXP-0061 (90-93)
    ("91", "FP-EXP-0061"),
    ("92", "FP-EXP-0061"),
    ("93", "FP-EXP-0061"),
    ("B4", "FP-EXP-0071"), // FP-EXP-0071 (B4-B7)
    ("B5", "FP-EXP-0071"),
    ("B6", "FP-EXP-0071"),
    ("B7", "FP-EXP-0071"),
    ("84", "FP-EXP-0081"), // FP-EXP-0081 (84-87)
    ("85", "FP-EXP-0081"),
    ("86", "FP-EXP-0081"),
    ("87", "FP-EXP-0081"),
    ("88", "FP-EXP-0091"), // FP-EXP-0091 (88-8B)
    ("89", "FP-EXP-0091"),
    ("8A", "FP-EXP-0091"),
    ("8B", "FP-EXP-0091"),
    ("30", "FP-EXP-1313"), // FP-EXP-1313 (30-33)
    ("31", "FP-EXP-1313"),
    ("32", "FP-EXP-1313"),
    ("33", "FP-EXP-1313"),
];

// Statically available map of firmware files per BoardType_Protocol key.
// Built once on first use by scanning ~/.fast/firmware (downloaded via check-updates if missing).
pub static AVAILABLE_FIRMWARE_VERSIONS: Lazy<HashMap<String, HashMap<String, String>>> =
    Lazy::new(|| build_available_firmware_versions());

// Helper: scan ~/.fast/firmware directory and build a map of BoardType_Protocol -> map of version -> file path.
fn build_available_firmware_versions() -> HashMap<String, HashMap<String, String>> {
    use std::fs;
    use std::path::PathBuf;

    let mut map: HashMap<String, HashMap<(u32, u32), String>> = HashMap::new();

    // Resolve firmware base directory under user's home
    let base: PathBuf = match directories::UserDirs::new() {
        Some(ud) => ud.home_dir().join(".fast").join("firmware"),
        None => PathBuf::from(""),
    };

    // If directory is missing or empty, trigger a download via check_updates
    let needs_download = match fs::read_dir(&base) {
        Ok(mut it) => it.next().is_none(),
        Err(_) => true,
    };
    if needs_download {
        let _ = crate::commands::check_updates::run();
    }

    let Ok(dir_iter) = fs::read_dir(&base) else {
        return HashMap::new();
    };

    for entry_res in dir_iter {
        if let Ok(entry) = entry_res {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let Ok(files) = fs::read_dir(&path) else {
                continue;
            };
            for file_res in files {
                if let Ok(file) = file_res {
                    let fpath = file.path();
                    if fpath
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.eq_ignore_ascii_case("txt"))
                        .unwrap_or(false)
                    {
                        if let Some(stem_os) = fpath.file_stem() {
                            if let Some(stem) = stem_os.to_str() {
                                // Expect pattern: {BoardType}_{Protocol}_firmware_v_{major}_{minor}
                                if let Some((prefix, ver_part_full)) =
                                    stem.split_once("_firmware_v_")
                                {
                                    if let Some((board_type, protocol)) = prefix.rsplit_once('_') {
                                        let mut it = ver_part_full.split('_');
                                        if let (Some(maj_s), Some(min_s)) = (it.next(), it.next()) {
                                            if let (Ok(maj), Ok(min)) =
                                                (maj_s.parse::<u32>(), min_s.parse::<u32>())
                                            {
                                                let key = format!("{}_{}", board_type, protocol);
                                                let version_key = (maj, min);
                                                let full_path = fpath.to_string_lossy().to_string();
                                                map.entry(key)
                                                    .or_default()
                                                    .entry(version_key)
                                                    .or_insert(full_path);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Convert (maj,min) keys to formatted version strings and ensure stable order when iterating consumers
    let mut out: HashMap<String, HashMap<String, String>> = HashMap::new();
    for (k, vers_map) in map.into_iter() {
        // sort by numeric (maj,min) by collecting and sorting
        let mut items: Vec<((u32, u32), String)> = vers_map.into_iter().collect();
        items.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        let mut inner: HashMap<String, String> = HashMap::new();
        for ((maj, min), path) in items {
            let ver_str = format!("{}.{}", maj, format!("{:02}", min));
            inner.insert(ver_str, path);
        }
        out.insert(k, inner);
    }
    out
}
