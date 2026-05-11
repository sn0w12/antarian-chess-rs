//! Self-install detection and shortcut creation.

use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::{env, fs};

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(target_os = "linux")]
const ICON_BYTES: &[u8] = include_bytes!("../assets/icons/icon.ico");

#[cfg(debug_assertions)]
pub(crate) fn should_prompt_for_install() -> bool {
    false
}

#[cfg(not(debug_assertions))]
pub(crate) fn should_prompt_for_install() -> bool {
    env::current_exe()
        .ok()
        .is_some_and(|current_exe| is_in_transient_location(&current_exe))
}

/// Returns true if the current executable is in a "temporary" or "download" location
/// and should be installed to a permanent home.
#[cfg_attr(debug_assertions, allow(dead_code))]
pub(crate) fn is_in_transient_location(exe_path: &Path) -> bool {
    let install_dir = get_install_dir().ok();
    if let Some(install_dir) = install_dir {
        // If we're already in the install dir, no need to prompt
        if exe_path.starts_with(&install_dir) {
            return false;
        }
    }

    true
}

/// Attempts to install the application to a permanent location if it's currently running from a transient location.
pub(crate) fn attempt_self_install() -> anyhow::Result<()> {
    let current_exe = env::current_exe()?;
    if !is_in_transient_location(&current_exe) {
        return Ok(()); // Already in a good location
    }

    let install_dir = get_install_dir()?;
    let target_path = install_dir.join(executable_name());

    // Copy (or move) the executable
    fs::create_dir_all(&install_dir)?;
    fs::copy(&current_exe, &target_path)?;
    // On Unix, preserve executable permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::metadata(&current_exe)?.permissions();
        fs::set_permissions(&target_path, perms)?;
    }

    // Create shortcuts / desktop entries
    create_shortcuts(&target_path)?;

    // Mark that we've installed (optional sentinel file)
    let sentinel = install_dir.join(".installed");
    fs::write(sentinel, env!("CARGO_PKG_VERSION"))?;

    // Install after copying and creating shortcuts
    // Relaunch with argument to delete the original file
    let original_path = current_exe.to_string_lossy().to_string();
    let mut cmd = Command::new(&target_path);
    cmd.arg(format!("--delete-old=\"{}\"", original_path));
    cmd.spawn()?;
    std::process::exit(0);
}

fn get_install_dir() -> anyhow::Result<PathBuf> {
    if cfg!(target_os = "windows") {
        // %LOCALAPPDATA%\Programs\antarian-chess-rs
        let local_app_data =
            dirs::data_local_dir().ok_or_else(|| anyhow::anyhow!("No local data directory"))?;
        Ok(local_app_data.join("Programs").join("antarian-chess-rs"))
    } else if cfg!(target_os = "linux") {
        // Linux & other Unix: ~/.local/bin
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("No home directory"))?;
        Ok(home.join(".local/bin"))
    } else {
        anyhow::bail!("Unsupported OS for installation")
    }
}

fn executable_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "antarian-chess-rs.exe"
    } else {
        "antarian-chess-rs"
    }
}

#[cfg(target_os = "windows")]
fn create_shortcuts(bin_path: &Path) -> anyhow::Result<()> {
    use std::fs;

    // Start Menu shortcut
    let start_menu_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("No config dir"))?
        .join(r"Microsoft\Windows\Start Menu\Programs");
    fs::create_dir_all(&start_menu_dir)?;
    let start_menu_link = start_menu_dir.join("antarian-chess-rs.lnk");
    create_lnk(bin_path, &start_menu_link)?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn create_lnk(target: &Path, link_path: &Path) -> anyhow::Result<()> {
    use std::process::Command;

    let target_str = target
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid target path"))?;
    let link_str = link_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid link path"))?;

    let ps = format!(
        "$WshShell = New-Object -comObject WScript.Shell; \
         $Shortcut = $WshShell.CreateShortcut('{}'); \
         $Shortcut.TargetPath = '{}'; \
         $Shortcut.Save()",
        link_str, target_str
    );
    let output = Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps])
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to create shortcut: {}", stderr)
    }
}

#[cfg(target_os = "linux")]
fn create_shortcuts(bin_path: &Path) -> anyhow::Result<()> {
    use image::ImageFormat;

    let cache_dir = dirs::cache_dir()
        .ok_or_else(|| anyhow::anyhow!("No cache dir"))?
        .join("antarian-chess-rs");
    fs::create_dir_all(&cache_dir)?;
    let png_path = cache_dir.join("icon.png");

    if !png_path.exists() {
        // Parse the .ico and take the first (largest) image
        let icon_dir = ico::IconDir::read(std::io::Cursor::new(ICON_BYTES))?;
        let icon_image = icon_dir
            .entries()
            .iter()
            .max_by_key(|e| e.width() * e.height())
            .ok_or_else(|| anyhow::anyhow!("No images in .ico"))?;
        let rgba = icon_image.decode()?;
        // Save as PNG
        image::save_buffer_with_format(
            &png_path,
            &rgba.rgba_data(),
            rgba.width(),
            rgba.height(),
            image::ColorType::Rgba8,
            ImageFormat::Png,
        )?;
    }

    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("No home dir"))?;
    let apps_dir = home.join(".local/share/applications");
    fs::create_dir_all(&apps_dir)?;
    let desktop_file = apps_dir.join("antarian-chess-rs.desktop");
    let icon_path_str = png_path.to_string_lossy();

    let content = format!(
        "[Desktop Entry]
Version=1.0
Type=Application
Name=antarian-chess-rs
Exec={}
Icon={}
Terminal=false
Categories=Development;Git;
Comment=Simple git GUI
",
        bin_path.display(),
        icon_path_str,
    );
    fs::write(&desktop_file, content)?;
    let _ = Command::new("update-desktop-database")
        .arg(&apps_dir)
        .output();

    Ok(())
}
