//! Updates from GitHub Releases: compare versions, download the installer, check its
//! SHA-256 against SHA256SUMS.txt, then run it silently and restart.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use tauri::{AppHandle, Emitter};

const REPO: &str = "Edgar-Mendonca/Datum-Schema";
/// No installer is anywhere near this size (they are about 4 MB).
const MAX_INSTALLER: u64 = 200 * 1024 * 1024;

#[derive(Serialize)]
pub struct Info {
    current: &'static str,
    latest: Option<String>,
    beta: bool,
    available: bool,
    notes: String,
    published: String,
    url: Option<String>,
    sums: Option<String>,
    size: u64,
    page: String,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(10))
        .timeout_read(std::time::Duration::from_secs(30))
        .try_proxy_from_env(true)
        .user_agent(concat!("DatumSchema/", env!("CARGO_PKG_VERSION")))
        .build()
}

/// 0.3.0-beta.2 → [0, 3, 0, 0, 2]; 0.3.0 → [0, 3, 0, 1, 0], so a release beats its betas.
fn parts(v: &str) -> Vec<u64> {
    let v = v.trim_start_matches('v');
    let (main, pre) = v.split_once('-').unwrap_or((v, ""));
    let mut out: Vec<u64> = main.split('.').map(|p| p.parse().unwrap_or(0)).collect();
    out.resize(3, 0);
    out.push(pre.is_empty() as u64);
    out.push(pre.rsplit('.').next().and_then(|n| n.parse().ok()).unwrap_or(0));
    out
}

/// The newest release; with `beta`, pre-releases count too.
pub fn check(beta: bool) -> Result<Info, String> {
    let current = env!("CARGO_PKG_VERSION");
    let page = format!("https://github.com/{REPO}/releases");
    let none = |page| Info { current, latest: None, beta: false, available: false, notes: String::new(), published: String::new(), url: None, sums: None, size: 0, page };
    let url = if beta { format!("https://api.github.com/repos/{REPO}/releases?per_page=10") } else { format!("https://api.github.com/repos/{REPO}/releases/latest") };
    let rel: serde_json::Value = match agent().get(&url).call() {
        Ok(r) => {
            let v: serde_json::Value = r.into_json().map_err(|e| e.to_string())?;
            match v.as_array() {
                Some(list) => match list.iter().filter(|r| !r["draft"].as_bool().unwrap_or(false)).max_by(|a, b| parts(a["tag_name"].as_str().unwrap_or("")).cmp(&parts(b["tag_name"].as_str().unwrap_or("")))) {
                    Some(r) => r.clone(),
                    None => return Ok(none(page)),
                },
                None => v,
            }
        }
        Err(ureq::Error::Status(404, _)) => return Ok(none(page)),
        Err(ureq::Error::Status(c, _)) => return Err(format!("GitHub answered {c}. Try again later.")),
        Err(_) => return Err("Could not reach GitHub. Check your internet connection.".into()),
    };
    let latest = rel["tag_name"].as_str().unwrap_or("").trim_start_matches('v').to_string();
    let assets = rel["assets"].as_array().cloned().unwrap_or_default();
    let find = |suffix: &str| assets.iter().find(|a| a["name"].as_str().is_some_and(|n| n.to_lowercase().ends_with(suffix)));
    let exe = find("-setup.exe");
    Ok(Info {
        current,
        available: !latest.is_empty() && parts(&latest) > parts(current),
        latest: Some(latest),
        beta: rel["prerelease"].as_bool().unwrap_or(false),
        notes: rel["body"].as_str().unwrap_or("").into(),
        published: rel["published_at"].as_str().unwrap_or("").get(..10).unwrap_or("").into(),
        url: exe.and_then(|a| a["browser_download_url"].as_str()).map(Into::into),
        sums: find("sha256sums.txt").and_then(|a| a["browser_download_url"].as_str()).map(Into::into),
        size: exe.and_then(|a| a["size"].as_u64()).unwrap_or(0),
        page: rel["html_url"].as_str().map(Into::into).unwrap_or(page),
    })
}

pub fn install(app: &AppHandle, url: &str, sums: Option<&str>) -> Result<(), String> {
    let base = format!("https://github.com/{REPO}/releases/download/");
    if !url.starts_with(&base) || sums.is_some_and(|s| !s.starts_with(&base)) {
        return Err("Updates are only installed from the Datum Schema releases page.".into());
    }
    // Every release publishes SHA256SUMS.txt; without it nothing is installed.
    let Some(sums) = sums else { return Err("This release has no checksum file, so it cannot be installed safely. Download it from the releases page.".into()) };
    if !cfg!(windows) {
        return Err("Automatic install is for Windows. Download the new version from the releases page.".into());
    }
    let name = url.rsplit('/').next().unwrap_or("").to_string();
    if !name.ends_with("-setup.exe") || !name.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)) || name.starts_with('.') {
        return Err("The installer's name is not what Datum Schema expects.".into());
    }
    // A new folder of its own, so nothing else can swap the file between the check and the start.
    let dir = std::env::temp_dir().join(format!("datumschema-update-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos())));
    std::fs::create_dir(&dir).map_err(|e| format!("Could not prepare the download: {e}"))?;
    let dest = dir.join(&name);
    let resp = agent().get(url).call().map_err(|e| format!("Download failed: {e}"))?;
    let total: u64 = resp.header("Content-Length").and_then(|v| v.parse().ok()).unwrap_or(0);
    let (mut src, mut file) = (resp.into_reader(), std::fs::File::create(&dest).map_err(|e| e.to_string())?);
    let (mut hash, mut buf, mut done, mut last) = (Sha256::new(), vec![0u8; 1 << 16], 0u64, 0u64);
    loop {
        let n = src.read(&mut buf).map_err(|e| format!("Download failed: {e}"))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        hash.update(&buf[..n]);
        done += n as u64;
        if done > MAX_INSTALLER {
            drop(file);
            let _ = std::fs::remove_dir_all(&dir);
            return Err("The download is far larger than an installer should be, so it was stopped and deleted.".into());
        }
        if total > 0 && done * 100 / total > last {
            last = done * 100 / total;
            let _ = app.emit("update-progress", last);
        }
    }
    drop(file);
    let fail = |e: String| format!("Could not fetch checksums: {e}");
    let list = agent().get(sums).call().map_err(|e| fail(e.to_string()))?.into_string().map_err(|e| fail(e.to_string()))?;
    // "<64 hex digits>  <file name>", the file name matched exactly.
    let want = list.lines().filter_map(|l| { let mut w = l.split_whitespace(); Some((w.next()?, w.next()?.trim_start_matches('*'))) }).find(|(_, f)| *f == name).map(|(h, _)| h).unwrap_or("");
    let got: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if want.len() != 64 || !want.eq_ignore_ascii_case(&got) {
        let _ = std::fs::remove_dir_all(&dir);
        return Err("The download did not match its checksum and was deleted. Try again.".into());
    }
    let _ = app.emit("update-progress", 100u64);
    launch(&dest)?;
    app.exit(0);
    Ok(())
}

/// Waits for this process to exit, installs silently, then starts the new version.
#[cfg(windows)]
fn launch(installer: &std::path::Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let line = format!(
        "/C ping -n 3 127.0.0.1 >nul & start \"\" /wait \"{}\" /S & start \"\" \"{}\"",
        installer.display(),
        exe.display()
    );
    std::process::Command::new("cmd")
        .raw_arg(line)
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .spawn()
        .map_err(|e| format!("Could not start the installer: {e}"))?;
    Ok(())
}

#[cfg(not(windows))]
fn launch(_: &std::path::Path) -> Result<(), String> {
    Ok(())
}
