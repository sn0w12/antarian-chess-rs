use std::env;
use std::process::Command;

const ICON_PATH: &str = "src/assets/icons/icon.ico";

#[cfg(windows)]
fn main() {
    let mut res = winres::WindowsResource::new();
    res.set_icon(ICON_PATH);
    res.compile().unwrap();

    set_about_info();
}

#[cfg(not(windows))]
fn main() {
    set_about_info();
}

fn set_about_info() {
    // Build info seen in about dialog.
    let commit_hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=GIT_COMMIT_HASH={}", commit_hash);

    let branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=GIT_BRANCH={}", branch);

    let build_time = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    println!("cargo:rustc-env=BUILD_TIMESTAMP={}", build_time);

    let rustc_version = Command::new("rustc")
        .args(["--version"])
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=RUSTC_VERSION={}", rustc_version);

    let target = env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=TARGET_TRIPLE={}", target);

    let profile = env::var("PROFILE").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=BUILD_PROFILE={}", profile);
}
