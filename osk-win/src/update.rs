//! Startup update check: asks GitHub for the latest release and, when it is
//! newer than this build, offers to download and run its installer.

use std::os::windows::process::CommandExt;
use std::process::Command;
use std::sync::atomic::{AtomicU8, Ordering};

use windows::core::HSTRING;
use windows::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, IDYES, MB_ICONINFORMATION, MB_OK, MB_SETFOREGROUND, MB_TOPMOST, MB_YESNO,
};

const REPO: &str = "juanfernandezzz/osk-win";
const INSTALLER: &str = "osk-win-setup.exe";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");
const IDLE: u8 = 0;
const BACKGROUND: u8 = 1;
const REQUESTED: u8 = 2;
static CHECKING: AtomicU8 = AtomicU8::new(IDLE);

struct CheckGuard<'a>(Option<&'a AtomicU8>);

impl<'a> CheckGuard<'a> {
    fn acquire(flag: &'a AtomicU8, manual: bool) -> Option<Self> {
        let request = if manual { REQUESTED } else { BACKGROUND };
        (flag.fetch_max(request, Ordering::AcqRel) == IDLE).then(|| Self(Some(flag)))
    }

    // A click either promotes this check or starts a new one after its
    // silent completion. Never clear a newer worker's state on drop.
    fn should_inform(&mut self) -> bool {
        let flag = self.0.expect("check still active");
        if flag.compare_exchange(BACKGROUND, IDLE, Ordering::AcqRel, Ordering::Acquire).is_ok() {
            self.0 = None;
            false
        } else {
            true
        }
    }
}

impl Drop for CheckGuard<'_> {
    fn drop(&mut self) {
        if let Some(flag) = self.0 { flag.store(IDLE, Ordering::Release); }
    }
}

fn inform(text: &str) {
    unsafe {
        MessageBoxW(None, &HSTRING::from(text), &HSTRING::from("osk-win"),
                    MB_OK | MB_ICONINFORMATION | MB_SETFOREGROUND | MB_TOPMOST);
    }
}

#[cfg(not(test))]
pub(crate) fn check_manually() {
    if let Some(guard) = CheckGuard::acquire(&CHECKING, true) {
        std::thread::spawn(move || run_check(guard));
    }
}

#[cfg(test)]
thread_local! {
    pub(crate) static MANUAL_CHECKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn check_manually() {
    MANUAL_CHECKS.with(|count| count.set(count.get() + 1));
}

fn run_check(mut guard: CheckGuard<'_>) {
    if let Err(e) = check(&mut guard) {
        warn!("osk: update check failed: {e}");
        if guard.should_inform() { inform(&format!("No se pudo comprobar si hay actualizaciones.\n\n{e}")); }
    }
}

/// Check in the background, so a slow or missing network never delays the
/// keyboard; a failed check only logs. It waits first: starting curl while
/// the window is still being drawn made the keyboard appear 5x slower.
pub fn check_at_startup() {
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(5));
        if let Some(guard) = CheckGuard::acquire(&CHECKING, false) {
            run_check(guard);
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

fn available_update(release: &serde_json::Value, current: &str) -> Result<Option<(String, String)>, String> {
    let tag = release["tag_name"].as_str().ok_or("no tag_name")?.trim_start_matches('v');
    if !is_newer(tag, current) {
        return Ok(None);
    }
    let prefix = format!("https://github.com/{REPO}/releases/download/");
    let url = release["assets"].as_array().into_iter().flatten()
        .find(|a| a["name"] == INSTALLER)
        .and_then(|a| a["browser_download_url"].as_str())
        .filter(|u| u.starts_with(&prefix)).ok_or("no installer in the release")?;
    Ok(Some((tag.to_owned(), url.to_owned())))
}

fn check(guard: &mut CheckGuard<'_>) -> Result<(), String> {
    let api = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let release: serde_json::Value =
        serde_json::from_slice(&curl(&["-m", "15", &api])?).map_err(|e| e.to_string())?;
    let current = VERSION;
    let Some((tag, url)) = available_update(&release, current)? else {
        if guard.should_inform() { inform(&format!("osk-win {current} está actualizado. No hay una versión más reciente disponible.")); }
        return Ok(());
    };
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
    curl(&["-m", "600", "-o", &setup.to_string_lossy(), &url])?;
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
    use super::*;

    #[test]
    fn current_and_older_releases_need_no_installer() {
        for tag in ["v0.3.0", "0.3.0", "v0.2.9"] {
            assert_eq!(available_update(&serde_json::json!({"tag_name":tag}), "0.3.0"), Ok(None));
        }
    }

    #[test]
    fn newer_release_selects_the_existing_windows_installer() {
        let url = format!("https://github.com/{REPO}/releases/download/v0.4.0/{INSTALLER}");
        let release = serde_json::json!({"tag_name":"v0.4.0", "assets":[
            {"name":"source.zip", "browser_download_url":"unrelated"},
            {"name":INSTALLER, "browser_download_url":url}
        ]});
        assert_eq!(available_update(&release, "0.3.0"), Ok(Some(("0.4.0".into(), url))));
    }

    #[test]
    fn incomplete_release_is_an_error_not_up_to_date() {
        for release in [serde_json::json!({}), serde_json::json!({"tag_name":"v0.4.0"})] {
            assert!(available_update(&release, "0.3.0").is_err());
        }
    }

    #[test]
    fn only_one_update_check_runs_and_completion_allows_retry() {
        let checking = AtomicU8::new(IDLE);
        let mut first = CheckGuard::acquire(&checking, false).unwrap();
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| assert!(CheckGuard::acquire(&checking, true).is_none()));
            }
        });
        assert!(first.should_inform(), "manual click gets the startup check's result");
        drop(first);
        let mut quiet = CheckGuard::acquire(&checking, false).unwrap();
        assert!(!quiet.should_inform());
        let mut next = CheckGuard::acquire(&checking, true).unwrap();
        drop(quiet);
        assert!(CheckGuard::acquire(&checking, false).is_none());
        assert!(next.should_inform());
    }

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
