use anyhow::Result;
#[cfg(not(windows))]
use anyhow::{Context, bail};
use std::path::PathBuf;

pub struct Session {
    pub url: String,
    pub data_dir: PathBuf,
    pub owner_token: Option<String>,
    pub lang: String,
}

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub fn run(session: Session) -> Result<()> {
    windows::run(session)
}

#[cfg(not(windows))]
pub fn run(session: Session) -> Result<()> {
    let Session {
        url,
        data_dir,
        owner_token,
        lang,
    } = session;
    drop((url, data_dir, owner_token, lang));
    bail!("The native service window is available on Windows")
}

#[cfg(windows)]
pub fn open_browser(url: &str) -> Result<()> {
    windows::open_browser(url)
}

#[cfg(not(windows))]
pub fn open_browser(url: &str) -> Result<()> {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(program)
        .arg(url)
        .spawn()
        .with_context(|| format!("Cannot open {url}. Open it in your browser."))?;
    Ok(())
}

#[cfg(windows)]
pub fn show_error(message: &str, lang: &str) {
    windows::show_error(message, lang);
}

#[cfg(not(windows))]
pub fn show_error(_message: &str, _lang: &str) {}

#[cfg(windows)]
pub fn close_window() {
    windows::close_window();
}

#[cfg(not(windows))]
pub fn close_window() {}
