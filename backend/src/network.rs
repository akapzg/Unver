use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;
use sysinfo::{Disks, Networks, System};

/// Check if container has host /proc mounted (for real host metrics)
fn has_host_proc() -> bool {
    Path::new("/host/proc/stat").exists()
}

/// ── Host CPU parsing (from /host/proc/stat) ──────────────────────────────

struct CpuSample {
    total: u64,
    idle: u64,
}

static LAST_CPU_SAMPLE: Mutex<Option<CpuSample>> = Mutex::new(None);

/// Read one CPU sample from /host/proc/stat, compute delta from last sample.
fn host_cpu_percent() -> Option<u8> {
    let stat = fs::read_to_string("/host/proc/stat").ok()?;
    let cpu_line = stat.lines().next()?;
    let fields: Vec<u64> = cpu_line
        .split_whitespace()
        .skip(1)
        .filter_map(|s| s.parse().ok())
        .collect();
    if fields.len() < 4 {
        return None;
    }
    let total: u64 = fields.iter().sum();
    let idle = fields[3] + fields.get(4).unwrap_or(&0);
    let current = CpuSample { total, idle };

    let mut last = LAST_CPU_SAMPLE.lock().ok()?;
    let pct = if let Some(ref prev) = *last {
        let dt = current.total.saturating_sub(prev.total);
        let di = current.idle.saturating_sub(prev.idle);
        if dt > 0 {
            Some(((dt - di) * 100 / dt) as u8)
        } else {
            None
        }
    } else {
        None
    };
    *last = Some(current);
    pct
}

/// ── Host memory parsing (from /host/proc/meminfo) ────────────────────────

fn host_memory() -> Option<(u64, u64)> {
    // Returns (used_kb, total_kb)
    let meminfo = fs::read_to_string("/host/proc/meminfo").ok()?;
    let mut total: Option<u64> = None;
    let mut available: Option<u64> = None;
    for line in meminfo.lines() {
        if line.starts_with("MemTotal:") {
            total = parse_kb(line);
        } else if line.starts_with("MemAvailable:") {
            available = parse_kb(line);
        }
        if total.is_some() && available.is_some() {
            break;
        }
    }
    let total = total?;
    let avail = available?;
    Some((total.saturating_sub(avail), total))
}

fn parse_kb(line: &str) -> Option<u64> {
    line.split_whitespace().nth(1)?.parse().ok()
}

/// ── Host uptime (from /host/proc/uptime) ─────────────────────────────────

pub fn host_uptime_secs() -> Option<u64> {
    let data = fs::read_to_string("/host/proc/uptime").ok()?;
    let secs: f64 = data.split_whitespace().next()?.parse().ok()?;
    Some(secs as u64)
}

/// ── Host disk usage (statvfs on /host) ───────────────────────────────────

fn host_disk_usage() -> Option<(u64, u64)> {
    // Returns (used_bytes, total_bytes)
    let path = std::ffi::CString::new("/host").ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(path.as_ptr(), &mut stat) } != 0 {
        return None;
    }
    let block_size = stat.f_frsize as u64;
    let total = stat.f_blocks as u64 * block_size;
    let available = stat.f_bavail as u64 * block_size;
    Some((total.saturating_sub(available), total))
}

// ════════════════════════════════════════════════════════════════════════════
// Public types
// ════════════════════════════════════════════════════════════════════════════

/// Real-time network rate tracking
pub struct NetTracker {
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_rate: f64, // bytes/sec
    pub tx_rate: f64,
    pub total_rx: u64,
    pub total_tx: u64,
    pub container_mode: bool,
}

impl NetTracker {
    pub fn new() -> Self {
        let container_mode = has_host_proc() || Path::new("/.dockerenv").exists();
        Self {
            rx_bytes: 0,
            tx_bytes: 0,
            rx_rate: 0.0,
            tx_rate: 0.0,
            total_rx: 0,
            total_tx: 0,
            container_mode,
        }
    }
}

pub type SharedNetTracker = std::sync::Arc<tokio::sync::RwLock<NetTracker>>;

/// Spawn a background task that polls network stats every 2 seconds.
pub async fn run_network_monitor(tracker: SharedNetTracker) {
    let mut sys = System::new_all();
    let mut nets = Networks::new_with_refreshed_list();

    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;

        sys.refresh_cpu_all();
        sys.refresh_memory();
        nets.refresh(true);

        let mut rx_total: u64 = 0;
        let mut tx_total: u64 = 0;
        for (_name, data) in nets.iter() {
            rx_total += data.total_received();
            tx_total += data.total_transmitted();
        }

        {
            let mut t = tracker.write().await;
            let prev_rx = t.rx_bytes;
            let prev_tx = t.tx_bytes;

            t.rx_rate = if prev_rx > 0 && rx_total > prev_rx {
                (rx_total - prev_rx) as f64 / 2.0
            } else {
                0.0
            };
            t.tx_rate = if prev_tx > 0 && tx_total > prev_tx {
                (tx_total - prev_tx) as f64 / 2.0
            } else {
                0.0
            };

            t.rx_bytes = rx_total;
            t.tx_bytes = tx_total;
            t.total_rx = rx_total;
            t.total_tx = tx_total;
        }
    }
}

/// Format bytes to human-readable
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;
    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    format!("{:.1} {}", size, UNITS[unit_idx])
}

/// Format rate (bytes/sec) to human-readable
pub fn format_rate(bytes_per_sec: f64) -> String {
    const UNITS: &[&str] = &["B/s", "KB/s", "MB/s", "GB/s"];
    let mut rate = bytes_per_sec;
    let mut unit_idx = 0;
    while rate >= 1024.0 && unit_idx < UNITS.len() - 1 {
        rate /= 1024.0;
        unit_idx += 1;
    }
    format!("{:.1} {}", rate, UNITS[unit_idx])
}

#[derive(serde::Serialize)]
pub struct SystemLoad {
    pub cpu_percent: u8,
    pub mem_used: u64,
    pub mem_total: u64,
    pub mem_percent: u8,
    pub disk_used: u64,
    pub disk_total: u64,
    pub disk_percent: u8,
}

/// Get CPU, memory, and disk usage — uses host metrics when /host/proc is mounted.
pub fn get_system_load() -> SystemLoad {
    if has_host_proc() {
        // ── Container mode: read host metrics ──
        let cpu = host_cpu_percent().unwrap_or(0);

        let (mem_used, mem_total) = host_memory().unwrap_or((0, 0));
        let mem_percent = if mem_total > 0 {
            ((mem_used as f64 / mem_total as f64) * 100.0).round() as u8
        } else {
            0
        };

        let (disk_used, disk_total) = host_disk_usage().unwrap_or((0, 0));
        let disk_percent = if disk_total > 0 {
            ((disk_used as f64 / disk_total as f64) * 100.0).round() as u8
        } else {
            0
        };

        return SystemLoad {
            cpu_percent: cpu,
            mem_used: mem_used * 1024, // KB → bytes
            mem_total: mem_total * 1024,
            mem_percent,
            disk_used,
            disk_total,
            disk_percent,
        };
    }

    // ── Native mode: use sysinfo ──
    let mut sys = System::new_all();
    sys.refresh_cpu_all();
    sys.refresh_memory();

    let cpu_usage = sys.global_cpu_usage();
    let cpu = cpu_usage.clamp(0.0, 100.0) as u8;
    let mem_total = sys.total_memory();
    let mem_used = sys.used_memory();

    let disks = Disks::new_with_refreshed_list();
    let disk_total: u64 = disks.iter().map(|d| d.total_space()).sum();
    let disk_used: u64 = disks
        .iter()
        .map(|d| d.total_space() - d.available_space())
        .sum();

    SystemLoad {
        cpu_percent: cpu,
        mem_used,
        mem_total,
        mem_percent: if mem_total > 0 {
            ((mem_used as f64 / mem_total as f64) * 100.0).round() as u8
        } else {
            0
        },
        disk_used,
        disk_total,
        disk_percent: if disk_total > 0 {
            ((disk_used as f64 / disk_total as f64) * 100.0).round() as u8
        } else {
            0
        },
    }
}
