//! Startup update check: asks GitHub for the latest release and, when it is
//! newer than this build, offers to download and run its installer.

use std::os::windows::process::CommandExt;
use std::process::Command;

use windows::core::HSTRING;
use windows::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, IDYES, MB_ICONINFORMATION, MB_SETFOREGROUND, MB_TOPMOST, MB_YESNO,
};

const REPO: &str = "juanfernandezzz/osk-win";
const INSTALLER: &str = "osk-win-setup.exe";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Check in the background, so a slow or missing network never delays the
/// keyboard; a failed check only logs. It waits first: starting curl while
/// the window is still being drawn made the keyboard appear 5x slower.
pub fn check_at_startup() {
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(5));
        if let Err(e) = check() {
            warn!("osk: update check failed: {e}");
        }
    });
}

/// `curl.exe` (standard on Windows 10/11) with `args`; its stdout.
fn curl(args: &[&str]) -> Result<Vec<u8>, String> {
    let out = Command::new("curl.exe")
        .args(["-sL", "--fail"])
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(format!("curl {}", out.status))
    }
}

/// Whether version `a` ("0.3.0") is newer than `b`.
fn is_newer(a: &str, b: &str) -> bool {
    let nums = |s: &str| s.split('.').map(|p| p.parse::<u32>().unwrap_or(0)).collect::<Vec<_>>();
    nums(a) > nums(b)
}

fn check() -> Result<(), String> {
    let api = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let release: serde_json::Value =
        serde_json::from_slice(&curl(&["-m", "15", &api])?).map_err(|e| e.to_string())?;
    let tag = release["tag_name"].as_str().ok_or("no tag_name")?.trim_start_matches('v');
    let current = env!("CARGO_PKG_VERSION");
    if !is_newer(tag, current) {
        return Ok(());
    }
    // The installer runs with administrator rights: only ever one of this repo's releases.
    let prefix = format!("https://github.com/{REPO}/releases/download/");
    let url = release["assets"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|a| a["name"] == INSTALLER)
        .and_then(|a| a["browser_download_url"].as_str())
        .filter(|u| u.starts_with(&prefix))
        .ok_or("no installer in the release")?;
    let text = format!(
        "Hay una versión nueva de osk-win ({tag}); tienes la {current}.\n\n\
         ¿Descargarla e instalarla ahora? El teclado se cierra durante la instalación."
    );
    let answer = unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(text),
            &HSTRING::from("osk-win"),
            MB_YESNO | MB_ICONINFORMATION | MB_TOPMOST | MB_SETFOREGROUND,
        )
    };
    if answer != IDYES {
        return Ok(());
    }
    let setup = std::env::temp_dir().join(INSTALLER);
    curl(&["-m", "600", "-o", &setup.to_string_lossy(), url])?;
    // `start` goes through ShellExecute, which raises the UAC prompt the installer needs.
    Command::new("cmd")
        .args(["/C", "start", ""])
        .arg(&setup)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn versions_compare_by_number_not_text() {
        assert!(is_newer("0.3.0", "0.2.0"));
        assert!(is_newer("0.10.0", "0.9.0"));
        assert!(is_newer("1.0.0", "0.99.9"));
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.1.9", "0.2.0"));
        assert!(!is_newer("garbage", "0.2.0"));
    }
}
