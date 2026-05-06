use std::sync::Arc;
use std::time::Duration;
use sysinfo::{Disks, Networks, System};
use tokio::sync::RwLock;

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
        // Check for /host/proc and /.dockerenv (Docker/container indicators)
        let container_mode = std::path::Path::new("/host/proc").exists()
            || std::path::Path::new("/.dockerenv").exists();
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

pub type SharedNetTracker = Arc<RwLock<NetTracker>>;

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

/// Get CPU and memory usage
pub fn get_system_load() -> SystemLoad {
    let mut sys = System::new_all();
    sys.refresh_cpu_all();
    sys.refresh_memory();

    // Average across all CPUs
    let cpu_usage = sys.global_cpu_usage();
    // sysinfo v0.33: global_cpu_usage returns 0.0-100.0
    let cpu = cpu_usage.clamp(0.0, 100.0) as u8;
    let mem_total = sys.total_memory();
    let mem_used = sys.used_memory();

    let disks = Disks::new_with_refreshed_list();
    let disk_total: u64 = disks.iter().map(|d| d.total_space()).sum();
    let disk_used: u64 = disks.iter().map(|d| d.total_space() - d.available_space()).sum();

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
