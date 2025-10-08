use std::path::{Path, PathBuf};

pub fn run() -> Result<(), String> {
    // Determine the user's home directory and target firmware storage under ~/.fast/firmware
    let user_dirs = directories::UserDirs::new().ok_or("could not determine user home directory")?;
    let target = user_dirs.home_dir().join(".fast").join("firmware");

    let url = "https://github.com/fastpinball/fast-firmware/archive/refs/heads/main.zip";
    println!("Downloading firmware archive from {} ...", url);
    let resp = reqwest::blocking::get(url).map_err(|e| format!("download failed: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP error: {}", resp.status()));
    }
    let bytes = resp.bytes().map_err(|e| format!("read body failed: {}", e))?;
    let reader = std::io::Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(reader).map_err(|e| format!("invalid zip: {}", e))?;

    std::fs::create_dir_all(&target).map_err(|e| format!("create target dir failed: {}", e))?;

    let mut extracted = 0usize;
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(|e| format!("zip read failed: {}", e))?;
        if file.is_dir() {
            continue;
        }
        let name_in_zip = file.name().to_string();
        // Skip top-level folder in GitHub zip (e.g., fast-firmware-main/)
        let rel_path: PathBuf = {
            let p = Path::new(&name_in_zip);
            let mut comps = p.components();
            let _top = comps.next();
            comps.collect()
        };
        if rel_path.as_os_str().is_empty() {
            continue;
        }
        // Only extract .txt firmware files
        if rel_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("txt"))
            .unwrap_or(false)
        {
            let out_path = target.join(&rel_path);
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("create dir failed: {}", e))?;
            }
            let mut out = std::fs::File::create(&out_path)
                .map_err(|e| format!("create file {} failed: {}", out_path.display(), e))?;
            std::io::copy(&mut file, &mut out)
                .map_err(|e| format!("write file {} failed: {}", out_path.display(), e))?;
            extracted += 1;
        }
    }
    if extracted == 0 {
        println!("No .txt firmware files were found in the archive.");
    } else {
        println!(
            "Downloaded and updated {} firmware files into {}.",
            extracted,
            target.display()
        );
    }
    Ok(())
}
