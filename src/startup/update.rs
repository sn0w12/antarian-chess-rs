//! Optional release update discovery and replacement logic.

#[cfg(feature = "auto_update")]
use std::env;
#[cfg(feature = "auto_update")]
use std::fs;
#[cfg(feature = "auto_update")]
use std::path::{Path, PathBuf};
#[cfg(feature = "auto_update")]
use std::process::Command;

#[cfg(feature = "auto_update")]
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(feature = "auto_update")]
use anyhow::{Context, bail};
#[cfg(feature = "auto_update")]
use reqwest::blocking::Client;
#[cfg(feature = "auto_update")]
use semver::Version;
#[cfg(feature = "auto_update")]
use serde::Deserialize;

#[cfg(feature = "auto_update")]
const USER: &str = "sn0w12";
#[cfg(feature = "auto_update")]
const REPO: &str = "antarian-chess-rs";

#[cfg(feature = "auto_update")]
#[derive(Debug, Deserialize)]
struct ReleaseResponse {
    tag_name: String,
    assets: Vec<ReleaseAsset>,
}

#[cfg(feature = "auto_update")]
#[derive(Debug, Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
}

/// Checks if a newer version of the application is available.
#[cfg(feature = "auto_update")]
pub(crate) fn has_update() -> Result<Version, bool> {
    let current_version = Version::parse(env!("CARGO_PKG_VERSION")).map_err(|_| false)?;
    let release = fetch_latest_release().map_err(|_| false)?;
    let latest_ver = Version::parse(release.tag_name.trim_start_matches('v')).map_err(|_| false)?;

    if latest_ver > current_version && release_asset(&release.assets, &latest_ver).is_ok() {
        Ok(latest_ver)
    } else {
        Err(false)
    }
}

/// Downloads the latest release and initiates the update process.
#[cfg(feature = "auto_update")]
pub(crate) fn download_update(version: &Version) -> anyhow::Result<()> {
    let release = fetch_latest_release()?;
    let release_version =
        Version::parse(release.tag_name.trim_start_matches('v')).context("invalid release tag")?;

    if &release_version != version {
        bail!(
            "Latest release changed while preparing update: expected {}, found {}",
            version,
            release_version
        );
    }

    let asset = release_asset(&release.assets, version)?;
    let download_bytes = Client::new()
        .get(&asset.browser_download_url)
        .header(
            "User-Agent",
            format!("{}-updater/{}", REPO, env!("CARGO_PKG_VERSION")),
        )
        .send()
        .context("failed to request update asset")?
        .error_for_status()
        .context("update download request failed")?
        .bytes()
        .context("failed to read update asset")?;

    let current_exe = env::current_exe().context("could not determine current executable")?;
    let download_path = temp_download_path(&asset.name);
    fs::write(&download_path, &download_bytes).context("failed to write downloaded update")?;
    set_executable_permissions(&download_path)?;
    spawn_update_replacer(&download_path, &current_exe)?;
    std::process::exit(0);
}

#[cfg(feature = "auto_update")]
fn fetch_latest_release() -> anyhow::Result<ReleaseResponse> {
    let url = format!("https://api.github.com/repos/{USER}/{REPO}/releases/latest");
    Client::new()
        .get(&url)
        .header(
            "User-Agent",
            format!("{}-updater/{}", REPO, env!("CARGO_PKG_VERSION")),
        )
        .send()
        .context("failed to query latest release")?
        .error_for_status()
        .context("latest release request failed")?
        .json::<ReleaseResponse>()
        .context("failed to parse latest release response")
}

#[cfg(feature = "auto_update")]
fn release_asset<'a>(
    assets: &'a [ReleaseAsset],
    version: &Version,
) -> anyhow::Result<&'a ReleaseAsset> {
    let asset_name = release_asset_name(version)?;
    assets
        .iter()
        .find(|asset| asset.name == asset_name)
        .with_context(|| format!("No update asset found for {}", asset_name))
}

#[cfg(feature = "auto_update")]
fn release_asset_name(version: &Version) -> anyhow::Result<String> {
    release_asset_name_for(env::consts::OS, env::consts::ARCH, version)
}

#[cfg(feature = "auto_update")]
fn release_asset_name_for(os: &str, arch: &str, version: &Version) -> anyhow::Result<String> {
    let version = version.to_string();
    match (os, arch) {
        ("windows", "x86_64") => Ok(format!("antarian-chess-rs-{version}-windows-x86_64.exe")),
        ("macos", "aarch64") => Ok(format!("antarian-chess-rs-{version}-macos-aarch64")),
        ("linux", "x86_64") => Ok(format!("antarian-chess-rs-{version}-linux-x86_64")),
        _ => bail!("Auto-update is not supported on {os}-{arch}"),
    }
}

#[cfg(feature = "auto_update")]
fn temp_download_path(asset_name: &str) -> PathBuf {
    let mut path = env::temp_dir();
    path.push(format!("antarian-chess-rs-update-{}-{asset_name}", std::process::id()));
    path
}

#[cfg(feature = "auto_update")]
fn set_executable_permissions(path: &Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path)
            .with_context(|| format!("failed to inspect {}", path.display()))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).with_context(|| {
            format!("failed to set executable permissions on {}", path.display())
        })?;
    }

    #[cfg(not(unix))]
    let _ = path;

    Ok(())
}

#[cfg(feature = "auto_update")]
fn spawn_update_replacer(download_path: &Path, target_path: &Path) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        let script = format!(
            "$source = '{source}'; \
             $target = '{target}'; \
             for ($attempt = 0; $attempt -lt 60; $attempt++) {{ \
                 Start-Sleep -Milliseconds 250; \
                 try {{ Copy-Item -Force $source $target; break }} catch {{ }} \
             }}; \
             Remove-Item -Force $source -ErrorAction SilentlyContinue; \
             Start-Process -FilePath $target",
            source = powershell_escape(download_path),
            target = powershell_escape(target_path)
        );

        Command::new("powershell")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-WindowStyle",
                "Hidden",
                "-Command",
                &script,
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .context("failed to launch Windows update helper")?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new("sh")
            .arg("-c")
            .arg(
                "source=\"$1\"; target=\"$2\"; \
                 for attempt in $(seq 1 60); do \
                     sleep 0.25; \
                     if cp \"$source\" \"$target\"; then break; fi; \
                 done; \
                 chmod +x \"$target\"; \
                 rm -f \"$source\"; \
                 \"$target\" >/dev/null 2>&1 &",
            )
            .arg("sh")
            .arg(download_path)
            .arg(target_path)
            .spawn()
            .context("failed to launch Unix update helper")?;
    }

    Ok(())
}

#[cfg(feature = "auto_update")]
#[cfg(target_os = "windows")]
fn powershell_escape(path: &Path) -> String {
    path.display().to_string().replace('\'', "''")
}

#[cfg(feature = "auto_update")]
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(all(test, feature = "auto_update"))]
mod tests {
    use super::release_asset_name_for;

    #[test]
    fn chooses_expected_release_asset_names() -> anyhow::Result<()> {
        let version = semver::Version::parse("0.1.5")?;

        assert_eq!(
            release_asset_name_for("linux", "x86_64", &version)?,
            "antarian-chess-rs-0.1.5-linux-x86_64"
        );
        assert_eq!(
            release_asset_name_for("macos", "aarch64", &version)?,
            "antarian-chess-rs-0.1.5-macos-aarch64"
        );
        assert_eq!(
            release_asset_name_for("windows", "x86_64", &version)?,
            "antarian-chess-rs-0.1.5-windows-x86_64.exe"
        );

        Ok(())
    }

    #[test]
    fn rejects_unsupported_targets() -> anyhow::Result<()> {
        let version = semver::Version::parse("0.1.5")?;
        assert!(release_asset_name_for("macos", "x86_64", &version).is_err());
        Ok(())
    }
}
