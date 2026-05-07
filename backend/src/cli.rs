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
}

/// Entry point — determine whether to run CLI command or start interactive menu / service.
pub async fn run() -> anyhow::Result<bool> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Start) => {
            // Fall through to start service
            return Ok(false);
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
    println!("  [0] Exit");
    println!();

    let choice = read_number("Select [0-5]: ");

    match choice {
        1 => {
            println!("Starting Unver service...");
            // Replace current process with `unver start`
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
    let arch = detect_arch();
    let asset_name = format!("unver-linux-{}.tar.gz", arch);

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

    let arch = detect_arch();
    let asset_name = format!("unver-linux-{}.tar.gz", arch);

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

    Ok(())
}

fn detect_arch() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "amd64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "arm64" // fallback (armv7 / OpenWrt use arm64 binary)
    }
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
