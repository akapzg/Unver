use std::io::{self, Write};
use std::process;

use clap::{Parser, Subcommand};

pub const GITHUB_RELEASES: &str = "https://api.github.com/repos/akapzg/Unver/releases/latest";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "unver", about = "Unver — Lightweight Reverse Proxy Manager", version = CURRENT_VERSION)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the Unver service (API + proxy engine)
    Start,
    /// Print version and exit
    Version,
    /// Self-update to the latest GitHub release
    Update,
    /// Restart the running Unver service
    Restart,
    /// Check if Unver service is running
    Status,
    /// Uninstall Unver — removes binary, static files, and init scripts (keeps data by default)
    Uninstall,
    /// Reset the admin password (forgot password recovery)
    ResetPassword,
}

/// Entry point — determine whether to run CLI command or start interactive menu / service.
pub async fn run() -> anyhow::Result<bool> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Start) => {
            // Fall through to start service
            Ok(false)
        }
        Some(Commands::Version) => {
            println!("unver v{}", CURRENT_VERSION);
            process::exit(0);
        }
        Some(Commands::Update) => {
            self_update().await?;
            process::exit(0);
        }
        Some(Commands::Restart) => {
            restart_service()?;
            process::exit(0);
        }
        Some(Commands::Status) => {
            show_status();
            process::exit(0);
        }
        Some(Commands::Uninstall) => {
            uninstall();
            process::exit(0);
        }
        Some(Commands::ResetPassword) => {
            reset_password().await?;
            process::exit(0);
        }
        None => {
            // No subcommand → interactive menu
            interactive_menu().await?;
            process::exit(0);
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Interactive Menu
// ════════════════════════════════════════════════════════════════════════════

async fn interactive_menu() -> anyhow::Result<()> {
    println!("═══════════════════════════════");
    println!("  Unver v{}", CURRENT_VERSION);
    println!("  Lightweight Reverse Proxy Manager");
    println!("═══════════════════════════════");
    println!("  [1] Start service");
    println!("  [2] Check status");
    println!("  [3] Restart service");
    println!("  [4] Self-update");
    println!("  [5] Version");
    println!("  [6] Reset admin password");
    println!("  [7] Uninstall");
    println!("  [0] Exit");
    println!();

    let choice = read_number("Select [0-7]: ");

    match choice {
        1 => {
            println!("Starting Unver service...");
            let current_exe = std::env::current_exe()?;
            let err = process::Command::new(current_exe)
                .arg("start")
                .spawn();
            match err {
                Ok(_) => println!("Service started in background."),
                Err(e) => eprintln!("Failed to start: {}", e),
            }
        }
        2 => show_status(),
        3 => restart_service()?,
        4 => self_update().await?,
        5 => println!("unver v{}", CURRENT_VERSION),
        6 => reset_password().await?,
        7 => uninstall(),
        0 => println!("Bye!"),
        _ => eprintln!("Invalid choice"),
    }
    Ok(())
}

fn read_number(prompt: &str) -> u8 {
    loop {
        print!("{}", prompt);
        io::stdout().flush().ok();
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            if let Ok(n) = input.trim().parse::<u8>() {
                return n;
            }
        }
        eprintln!("Invalid input, try again.");
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Self-update
// ════════════════════════════════════════════════════════════════════════════

async fn self_update() -> anyhow::Result<()> {
    println!("Checking for updates...");

    let client = reqwest::Client::builder()
        .user_agent("unver-updater")
        .build()?;

    let resp: serde_json::Value = client
        .get(GITHUB_RELEASES)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?
        .json()
        .await?;

    let latest = resp["tag_name"]
        .as_str()
        .and_then(|t| t.strip_prefix('v'))
        .unwrap_or("unknown");

    if latest == CURRENT_VERSION {
        println!("Already up-to-date (v{})", CURRENT_VERSION);
        return Ok(());
    }

    println!("New version available: v{} (current: v{})", latest, CURRENT_VERSION);

    // Detect architecture
    let (platform, arch) = detect_platform();
    let asset_name = format!("unver-{}-{}.tar.gz", platform, arch);

    // Find download URL
    let assets = resp["assets"].as_array()
        .ok_or_else(|| anyhow::anyhow!("No assets in release"))?;

    let download_url = assets
        .iter()
        .find_map(|a| {
            let name = a["name"].as_str()?;
            if name == asset_name {
                a["browser_download_url"].as_str()
            } else {
                None
            }
        })
        .ok_or_else(|| anyhow::anyhow!("No asset matching {}", asset_name))?;

    println!("Downloading {}...", asset_name);

    let bytes = client.get(download_url).send().await?.bytes().await?;

    // Extract
    let tmpdir = tempfile::tempdir()?;
    let tar_path = tmpdir.path().join("unver.tar.gz");
    std::fs::write(&tar_path, &bytes)?;

    // Decompress and extract
    let tar_file = std::fs::File::open(&tar_path)?;
    let decoder = flate2::read::GzDecoder::new(tar_file);
    let mut archive = tar::Archive::new(decoder);

    let extract_dir = tmpdir.path().join("extracted");
    std::fs::create_dir_all(&extract_dir)?;
    archive.unpack(&extract_dir)?;

    let new_binary = extract_dir.join("unver");
    if !new_binary.exists() {
        anyhow::bail!("Binary not found in archive");
    }

    // Atomic replace: write to temp, then rename
    let current_exe = std::env::current_exe()?;
    let parent = current_exe.parent().unwrap();
    let tmp_exe = parent.join(".unver.new");

    std::fs::copy(&new_binary, &tmp_exe)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp_exe, std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::rename(&tmp_exe, &current_exe)?;

    // Also update static/ directory from the release tarball (atomic via rename)
    let new_static = extract_dir.join("static");
    if new_static.exists() && new_static.is_dir() {
        let target_static = parent.join("static");
        let tmp_static = parent.join(".static.new");
        // Clean up any leftover from a previous failed update
        if tmp_static.exists() {
            std::fs::remove_dir_all(&tmp_static)?;
        }
        copy_dir_all(&new_static, &tmp_static)?;
        // Atomic swap: rename old out, rename new in
        let old_static = parent.join(".static.old");
        if target_static.exists() {
            if old_static.exists() {
                std::fs::remove_dir_all(&old_static)?;
            }
            std::fs::rename(&target_static, &old_static)?;
        }
        std::fs::rename(&tmp_static, &target_static)?;
        // Clean up old
        if old_static.exists() {
            let _ = std::fs::remove_dir_all(&old_static);
        }
    }

    println!("Updated to v{}! Restart to apply: unver restart", latest);
    Ok(())
}

/// Programmatic self-update (no console output, for API use).
pub async fn self_update_programmatic() -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .user_agent("unver-updater")
        .build()?;

    let resp: serde_json::Value = client
        .get(GITHUB_RELEASES)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?
        .json()
        .await?;

    let latest = resp["tag_name"]
        .as_str()
        .and_then(|t| t.strip_prefix('v'))
        .unwrap_or("unknown");

    // Skip if already latest (shouldn't happen since API already checked)
    if latest == CURRENT_VERSION {
        anyhow::bail!("Already up-to-date (v{})", CURRENT_VERSION);
    }

    let (platform, arch) = detect_platform();
    let asset_name = format!("unver-{}-{}.tar.gz", platform, arch);

    let assets = resp["assets"].as_array()
        .ok_or_else(|| anyhow::anyhow!("No assets in release"))?;

    let download_url = assets
        .iter()
        .find_map(|a| {
            let name = a["name"].as_str()?;
            if name == asset_name {
                a["browser_download_url"].as_str()
            } else {
                None
            }
        })
        .ok_or_else(|| anyhow::anyhow!("No asset matching {}", asset_name))?;

    let bytes = client.get(download_url).send().await?.bytes().await?;

    let tmpdir = tempfile::tempdir()?;
    let tar_path = tmpdir.path().join("unver.tar.gz");
    std::fs::write(&tar_path, &bytes)?;

    let tar_file = std::fs::File::open(&tar_path)?;
    let decoder = flate2::read::GzDecoder::new(tar_file);
    let mut archive = tar::Archive::new(decoder);

    let extract_dir = tmpdir.path().join("extracted");
    std::fs::create_dir_all(&extract_dir)?;
    archive.unpack(&extract_dir)?;

    let new_binary = extract_dir.join("unver");
    if !new_binary.exists() {
        anyhow::bail!("Binary not found in archive");
    }

    let current_exe = std::env::current_exe()?;
    let parent = current_exe.parent().unwrap();
    let tmp_exe = parent.join(".unver.new");

    std::fs::copy(&new_binary, &tmp_exe)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp_exe, std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::rename(&tmp_exe, &current_exe)?;

    // Also update static/ directory from the release tarball (atomic via rename)
    let new_static = extract_dir.join("static");
    if new_static.exists() && new_static.is_dir() {
        let target_static = parent.join("static");
        let tmp_static = parent.join(".static.new");
        // Clean up any leftover from a previous failed update
        if tmp_static.exists() {
            std::fs::remove_dir_all(&tmp_static)?;
        }
        copy_dir_all(&new_static, &tmp_static)?;
        // Atomic swap: rename old out, rename new in
        let old_static = parent.join(".static.old");
        if target_static.exists() {
            if old_static.exists() {
                std::fs::remove_dir_all(&old_static)?;
            }
            std::fs::rename(&target_static, &old_static)?;
        }
        std::fs::rename(&tmp_static, &target_static)?;
        // Clean up old
        if old_static.exists() {
            let _ = std::fs::remove_dir_all(&old_static);
        }
    }

    Ok(())
}

fn detect_platform() -> (&'static str, &'static str) {
    let arch = if cfg!(target_arch = "x86_64") {
        "amd64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "arm64" // fallback (armv7 / OpenWrt use arm64 binary)
    };
    let platform = if cfg!(target_env = "musl") { "openwrt" } else { "linux" };
    (platform, arch)
}

// ════════════════════════════════════════════════════════════════════════════
// Service management
// ════════════════════════════════════════════════════════════════════════════

fn restart_service() -> anyhow::Result<()> {
    // Find unver PID via pgrep
    let current_pid = std::process::id();
    let current_exe = std::env::current_exe()?;
    let current_path = current_exe.to_string_lossy();

    // Read /proc to find running unver processes, excluding ourselves
    let pids = find_unver_pids(&current_path, current_pid);

    if pids.is_empty() {
        println!("No running Unver service found.");
        println!("Start it with: unver start");
        return Ok(());
    }

    for pid in &pids {
        println!("Restarting Unver (PID {})...", pid);
        unsafe { libc::kill(*pid as i32, libc::SIGTERM) };
    }

    println!("Service restart signal sent. The service manager (systemd/procd) will restart it.");
    Ok(())
}

fn show_status() {
    let current_pid = std::process::id();
    let current_exe = std::env::current_exe().unwrap_or_default();
    let current_path = current_exe.to_string_lossy();

    let pids = find_unver_pids(&current_path, current_pid);

    if pids.is_empty() {
        println!("Unver service: not running");
        return;
    }

    println!("Unver service: running");
    for pid in &pids {
        println!("  PID: {}", pid);

        // Read /proc/{pid}/status for uptime
        if let Ok(status) = std::fs::read_to_string(format!("/proc/{}/status", pid)) {
            if let Some(name) = status.lines().find(|l| l.starts_with("Name:")) {
                println!("  {}", name);
            }
        }

        // Read cmdline
        if let Ok(cmdline) = std::fs::read_to_string(format!("/proc/{}/cmdline", pid)) {
            let cmd = cmdline.replace('\0', " ");
            println!("  Command: {}", cmd.trim());
        }
    }

    println!("  Version: v{}", CURRENT_VERSION);
}

fn find_unver_pids(exclude_path: &str, exclude_pid: u32) -> Vec<u32> {
    let mut pids = Vec::new();
    let proc_dir = match std::fs::read_dir("/proc") {
        Ok(d) => d,
        Err(_) => return pids,
    };

    for entry in proc_dir.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let pid: u32 = match name_str.parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        if pid == exclude_pid {
            continue;
        }

        // Check exe symlink
        if let Ok(exe) = std::fs::read_link(format!("/proc/{}/exe", pid)) {
            let exe_str = exe.to_string_lossy();
            if exe_str.contains("unver") || exe_str == exclude_path {
                pids.push(pid);
            }
        }
    }
    pids
}

// ════════════════════════════════════════════════════════════════════════════
// Reset password
// ════════════════════════════════════════════════════════════════════════════

async fn reset_password() -> anyhow::Result<()> {
    use std::io::{self, Write};
    use sqlx::SqlitePool;

    let config = crate::config::Config::load()?;
    let db_path = config.database_path();

    if !db_path.exists() {
        anyhow::bail!("Database not found at {}. Is Unver installed?", db_path.display());
    }

    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
    let pool = SqlitePool::connect(&db_url).await?;

    // Find the admin user
    let user = sqlx::query!("SELECT id, username FROM users LIMIT 1")
        .fetch_optional(&pool)
        .await?;

    let (user_id, username) = match user {
        Some(u) => (u.id.unwrap_or_default(), u.username),
        None => {
            anyhow::bail!("No admin user found. Run setup first.");
        }
    };

    println!("Resetting password for admin user: {}", username);

    // Prompt for new password
    print!("New password (min 8 chars): ");
    io::stdout().flush().ok();
    let mut password = String::new();
    io::stdin().read_line(&mut password)?;
    let password = password.trim().to_string();

    print!("Confirm password: ");
    io::stdout().flush().ok();
    let mut confirm = String::new();
    io::stdin().read_line(&mut confirm)?;

    if password != confirm.trim() {
        println!("Passwords do not match.");
        return Ok(());
    }

    if password.len() < 8 {
        println!("Password must be at least 8 characters.");
        return Ok(());
    }

    let password_hash = crate::security::hash_password(&password)?;
    sqlx::query!("UPDATE users SET password_hash = ?, updated_at = datetime('now') WHERE id = ?",
        password_hash, user_id)
        .execute(&pool)
        .await?;

    // Invalidate all refresh tokens
    sqlx::query!("DELETE FROM refresh_tokens WHERE user_id = ?", user_id)
        .execute(&pool)
        .await?;

    println!("Password reset successfully. You can now log in with the new password.");
    Ok(())
}

fn uninstall() {
    println!("Uninstalling Unver...");
    println!();

    let current_exe = std::env::current_exe().unwrap_or_default();
    let current_path = current_exe.to_string_lossy();
    let exe_dir = current_exe.parent().map(|p| p.to_path_buf()).unwrap_or_default();

    // 1. Stop running services
    let pids = find_unver_pids(&current_path, std::process::id());
    for pid in &pids {
        println!("Stopping Unver (PID {})...", pid);
        unsafe { libc::kill(*pid as i32, libc::SIGTERM); }
    }
    if !pids.is_empty() {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    // 2. Remove init scripts
    // systemd
    let systemd_unit = "/etc/systemd/system/unver.service";
    if std::path::Path::new(systemd_unit).exists() {
        let _ = process::Command::new("systemctl")
            .args(["stop", "unver"])
            .output();
        let _ = process::Command::new("systemctl")
            .args(["disable", "unver"])
            .output();
        if std::fs::remove_file(systemd_unit).is_ok() {
            println!("Removed: {}", systemd_unit);
        }
        let _ = process::Command::new("systemctl").arg("daemon-reload").output();
    }

    // procd (OpenWrt)
    let procd_unit = "/etc/init.d/unver";
    if std::path::Path::new(procd_unit).exists() {
        let _ = process::Command::new(procd_unit)
            .arg("stop")
            .output();
        let _ = process::Command::new(procd_unit)
            .arg("disable")
            .output();
        if std::fs::remove_file(procd_unit).is_ok() {
            println!("Removed: {}", procd_unit);
        }
    }

    // 3. Remove binary and static files
    let binary = exe_dir.join("unver");
    if binary.exists()
        && std::fs::remove_file(&binary).is_ok() {
            println!("Removed: {}", binary.display());
        }

    let static_dir = exe_dir.join("static");
    if static_dir.exists()
        && std::fs::remove_dir_all(&static_dir).is_ok() {
            println!("Removed: {}", static_dir.display());
        }

    println!();
    println!("Uninstall complete.");
    println!("Data directory was preserved. To remove it manually:");
    println!("  rm -rf /var/lib/unver      (Linux)");
    println!("  rm -rf /etc/unver          (OpenWrt)");
}

/// Recursively copy a directory.
fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let target = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}
