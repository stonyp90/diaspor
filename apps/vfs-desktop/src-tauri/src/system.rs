//! System Metrics Collection Module
//! 
//! Provides CPU, memory, and system information using sysinfo crate.

use serde::{Deserialize, Serialize};
use sysinfo::{System, Networks, Disks};
use std::sync::Mutex;
use std::time::Instant;
use once_cell::sync::Lazy;
use tracing::{debug, warn};

/// State for calculating rates (bytes per second)
struct IoState {
    networks: Networks,
    disks: Disks,
    last_update: Instant,
    last_net_rx: u64,
    last_net_tx: u64,
    last_disk_read: u64,
    last_disk_write: u64,
    // Calculated rates
    net_rx_rate: u64,
    net_tx_rate: u64,
    disk_read_rate: u64,
    disk_write_rate: u64,
}

impl IoState {
    fn new() -> Self {
        let networks = Networks::new_with_refreshed_list();
        let (net_rx, net_tx) = get_network_totals_from(&networks);
        
        let disks = Disks::new_with_refreshed_list();
        let (disk_read, disk_write) = get_disk_totals_from(&disks);
        
        Self {
            networks,
            disks,
            last_update: Instant::now(),
            last_net_rx: net_rx,
            last_net_tx: net_tx,
            last_disk_read: disk_read,
            last_disk_write: disk_write,
            net_rx_rate: 0,
            net_tx_rate: 0,
            disk_read_rate: 0,
            disk_write_rate: 0,
        }
    }
}

static IO_STATE: Lazy<Mutex<IoState>> = Lazy::new(|| Mutex::new(IoState::new()));

/// System information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os_name: String,
    pub os_version: String,
    pub kernel_version: String,
    pub hostname: String,
    pub cpu_brand: String,
    pub cpu_cores: usize,
    pub total_memory_mb: u64,
    pub total_swap_mb: u64,
}

/// Real-time system metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub cpu_usage: f32,
    pub per_core_usage: Vec<f32>,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub memory_usage_percent: f32,
    pub swap_used_mb: u64,
    pub swap_total_mb: u64,
    pub disk_read_bytes_sec: u64,
    pub disk_write_bytes_sec: u64,
    pub network_rx_bytes_sec: u64,
    pub network_tx_bytes_sec: u64,
    pub load_average: [f64; 3],
    pub uptime_seconds: u64,
    pub timestamp: u64,
}

/// Process information for running models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_usage: f32,
    pub memory_mb: u64,
    pub status: String,
    pub start_time: u64,
}

/// Get system information (called once at startup - can use full refresh)
pub fn get_system_info() -> SystemInfo {
    let mut sys = System::new();
    sys.refresh_cpu_all(); // Only refresh CPU for brand info
    sys.refresh_memory(); // Only refresh memory for totals

    let cpu_cores = sys.cpus().len();
    let cpu_brand = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "Unknown CPU".to_string());

    SystemInfo {
        os_name: System::name().unwrap_or_else(|| "Unknown".to_string()),
        os_version: System::os_version().unwrap_or_else(|| "Unknown".to_string()),
        kernel_version: System::kernel_version().unwrap_or_else(|| "Unknown".to_string()),
        hostname: System::host_name().unwrap_or_else(|| "Unknown".to_string()),
        cpu_brand,
        cpu_cores,
        total_memory_mb: sys.total_memory() / (1024 * 1024),
        total_swap_mb: sys.total_swap() / (1024 * 1024),
    }
}

/// Global System instance for reuse (avoids expensive re-initialization)
static SYSTEM_INSTANCE: once_cell::sync::Lazy<parking_lot::Mutex<System>> =
    once_cell::sync::Lazy::new(|| parking_lot::Mutex::new(System::new()));

/// Get current system metrics (optimized for frequent polling)
pub fn get_system_metrics() -> SystemMetrics {
    let mut sys = SYSTEM_INSTANCE.lock();
    
    // Only refresh what we need (much faster than refresh_all())
    sys.refresh_cpu_usage(); // CPU usage only
    sys.refresh_memory(); // Memory usage only
    // Don't refresh processes, disks, networks - we handle those separately

    let cpu_usage = sys.global_cpu_usage();
    let per_core_usage: Vec<f32> = sys.cpus().iter().map(|c| c.cpu_usage()).collect();

    let memory_used = sys.used_memory() / (1024 * 1024);
    let memory_total = sys.total_memory() / (1024 * 1024);
    let memory_usage_percent = if memory_total > 0 {
        (memory_used as f32 / memory_total as f32) * 100.0
    } else {
        0.0
    };

    // Get disk I/O and network I/O rates (uses persistent state)
    let (disk_read, disk_write, net_rx, net_tx) = calculate_io_rates();

    let load_avg = System::load_average();

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    SystemMetrics {
        cpu_usage,
        per_core_usage,
        memory_used_mb: memory_used,
        memory_total_mb: memory_total,
        memory_usage_percent,
        swap_used_mb: sys.used_swap() / (1024 * 1024),
        swap_total_mb: sys.total_swap() / (1024 * 1024),
        disk_read_bytes_sec: disk_read,
        disk_write_bytes_sec: disk_write,
        network_rx_bytes_sec: net_rx,
        network_tx_bytes_sec: net_tx,
        load_average: [load_avg.one, load_avg.five, load_avg.fifteen],
        uptime_seconds: System::uptime(),
        timestamp,
    }
}

/// Get network cumulative totals from a Networks reference
fn get_network_totals_from(networks: &Networks) -> (u64, u64) {
    let mut rx = 0u64;
    let mut tx = 0u64;

    for (_, data) in networks.iter() {
        rx += data.received();
        tx += data.transmitted();
    }

    (rx, tx)
}

/// Get disk I/O cumulative totals (platform-specific implementation)
/// 
/// On macOS, uses `iostat -I -d` to get cumulative disk I/O statistics since boot.
/// The cumulative MB transferred is tracked over time, and the difference between
/// calls is used to calculate bytes per second rates in `calculate_io_rates()`.
/// 
/// Note: `iostat -I` combines read and write, so both values will be the same.
/// For separate read/write tracking, a different approach would be needed.
#[cfg(target_os = "macos")]
fn get_disk_totals_from(_disks: &Disks) -> (u64, u64) {
    use std::process::Command;
    
    // On macOS, use iostat -I -d to get cumulative disk I/O statistics since boot
    // Format: device KB/t xfrs MB (cumulative MB transferred)
    // Example output:
    //               disk0               disk4 
    //     KB/t xfrs   MB     KB/t xfrs   MB 
    //     26.66 59194931 1541240.99   133.54  89 11.61
    if let Ok(output) = Command::new("iostat")
        .args(["-I", "-d"])
        .output()
    {
        if output.status.success() {
            if let Ok(stdout) = String::from_utf8(output.stdout) {
                // Parse iostat output
                let mut total_read_mb = 0.0;
                let mut total_write_mb = 0.0;
                let mut found_data = false;
                let _past_header = false;
                
                // Parse iostat output
                // Format:
                //   Line 1: device names (disk0, disk4, etc.) - may be empty
                //   Line 2: column headers (KB/t xfrs MB)
                //   Line 3+: data values (KB/t xfrs MB [KB/t xfrs MB] ...)
                // Example:
                //              disk0               disk4 
                //    KB/t xfrs   MB     KB/t xfrs   MB 
                //    27.44 60679562 1625867.05   129.32  92 11.62
                
                let lines: Vec<&str> = stdout.lines().collect();
                
                // Find the data line (first line that starts with a number after skipping headers)
                for line in &lines {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    // Skip header lines
                    if trimmed.contains("KB/t") || trimmed.contains("disk0") || trimmed.contains("disk1") || trimmed.contains("disk2") || trimmed.contains("disk3") || trimmed.contains("disk4") {
                        continue;
                    }
                    // Check if this line starts with a number (data line)
                    if trimmed.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                        let parts: Vec<&str> = trimmed.split_whitespace().collect();
                        
                        debug!("[Disk I/O] Parsing data line: {}", trimmed);
                        debug!("[Disk I/O] Found {} parts", parts.len());
                        
                        // Process each device (every 3 values = one device)
                        let mut i = 0;
                        while i + 2 < parts.len() {
                            // Each device has: KB/t (parts[i]), xfrs (parts[i+1]), MB (parts[i+2])
                            if let Ok(mb) = parts[i + 2].parse::<f64>() {
                                // iostat -I combines read+write, so split 50/50 as approximation
                                total_read_mb += mb * 0.5;
                                total_write_mb += mb * 0.5;
                                found_data = true;
                                debug!("[Disk I/O] Found device {}: {} MB cumulative (split 50/50)", i / 3, mb);
                            } else {
                                warn!("[Disk I/O] Failed to parse MB value: {}", parts[i + 2]);
                            }
                            i += 3; // Move to next device (skip KB/t, xfrs, MB)
                        }
                        break; // Found and processed data line, exit loop
                    }
                }
                
                if found_data {
                    // Convert cumulative MB to cumulative bytes
                    let cumulative_read_bytes = (total_read_mb * 1024.0 * 1024.0) as u64;
                    let cumulative_write_bytes = (total_write_mb * 1024.0 * 1024.0) as u64;
                    debug!("[Disk I/O] Total cumulative: read={} bytes, write={} bytes", cumulative_read_bytes, cumulative_write_bytes);
                    return (cumulative_read_bytes, cumulative_write_bytes);
                } else {
                    warn!("[Disk I/O] iostat output parsed but no disk data found");
                }
            } else {
                warn!("[Disk I/O] Failed to parse iostat output as UTF-8");
            }
        } else {
            warn!("[Disk I/O] iostat command failed with status: {:?}", output.status);
        }
    } else {
        warn!("[Disk I/O] Failed to execute iostat command");
    }
    
    // Fallback: return 0 if iostat is not available
    // The rate calculation will handle this gracefully
    (0, 0)
}

/// Get disk I/O cumulative totals (platform-specific implementation)
/// 
/// On Linux, reads from /proc/diskstats which provides cumulative sector counts
/// for all physical disk devices. Sectors are converted to bytes (512 bytes/sector).
/// This is the most reliable method on Linux.
#[cfg(target_os = "linux")]
fn get_disk_totals_from(_disks: &Disks) -> (u64, u64) {
    use std::fs;
    
    // Read /proc/diskstats - the reliable way on Linux
    if let Ok(content) = fs::read_to_string("/proc/diskstats") {
        let mut read_sectors = 0u64;
        let mut write_sectors = 0u64;
        
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 14 {
                // Skip loop and dm- devices
                let name = parts.get(2).unwrap_or(&"");
                if name.starts_with("loop") || name.starts_with("dm-") {
                    continue;
                }
                // Only include real disks (sda, nvme0n1, etc.)
                if name.starts_with("sd") || name.starts_with("nvme") || name.starts_with("vd") || name.starts_with("hd") {
                    read_sectors += parts.get(5).and_then(|s| s.parse().ok()).unwrap_or(0);
                    write_sectors += parts.get(9).and_then(|s| s.parse().ok()).unwrap_or(0);
                }
            }
        }
        
        // Sector size is typically 512 bytes
        return (read_sectors * 512, write_sectors * 512);
    }
    (0, 0)
}

#[cfg(target_os = "windows")]
fn get_disk_totals_from(_disks: &Disks) -> (u64, u64) {
    use crate::vfs::platform::CommandBuilder;
    use std::fs;
    
    // Method 1: Try to read from Windows performance data via PowerShell (cumulative bytes)
    // Use a simpler command that returns JSON for easier parsing
    let ps_command = r#"
        $counters = Get-Counter '\PhysicalDisk(_Total)\Disk Read Bytes/sec','\PhysicalDisk(_Total)\Disk Write Bytes/sec' -ErrorAction SilentlyContinue
        if ($counters) {
            $read = ($counters.CounterSamples | Where-Object { $_.Path -like '*Read*' }).CookedValue
            $write = ($counters.CounterSamples | Where-Object { $_.Path -like '*Write*' }).CookedValue
            "$read`n$write"
        }
    "#;
    
    if let Ok(output) = CommandBuilder::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", ps_command])
        .output()
    {
        if output.status.success() {
            if let Ok(stdout) = String::from_utf8(output.stdout) {
                let lines: Vec<&str> = stdout.trim().lines().collect();
                if lines.len() >= 2 {
                    // Get-Counter returns instantaneous rates in bytes/sec
                    // We'll use these directly as approximate byte counts for rate calculation
                    let read_rate: f64 = lines[0].trim().parse().unwrap_or(0.0);
                    let write_rate: f64 = lines[1].trim().parse().unwrap_or(0.0);
                    
                    // For rate calculation, we need cumulative totals
                    // Use a static accumulator to simulate cumulative values
                    use std::sync::atomic::{AtomicU64, Ordering};
                    static CUMULATIVE_READ: AtomicU64 = AtomicU64::new(0);
                    static CUMULATIVE_WRITE: AtomicU64 = AtomicU64::new(0);
                    static LAST_UPDATE: once_cell::sync::Lazy<parking_lot::Mutex<std::time::Instant>> = 
                        once_cell::sync::Lazy::new(|| parking_lot::Mutex::new(std::time::Instant::now()));
                    
                    let mut last = LAST_UPDATE.lock();
                    let elapsed = last.elapsed().as_secs_f64();
                    *last = std::time::Instant::now();
                    
                    // Accumulate bytes based on rate * elapsed time
                    let read_bytes = (read_rate * elapsed.max(0.1)) as u64;
                    let write_bytes = (write_rate * elapsed.max(0.1)) as u64;
                    
                    let total_read = CUMULATIVE_READ.fetch_add(read_bytes, Ordering::SeqCst) + read_bytes;
                    let total_write = CUMULATIVE_WRITE.fetch_add(write_bytes, Ordering::SeqCst) + write_bytes;
                    
                    return (total_read, total_write);
                }
            }
        }
    }
    
    // Method 2: Try to read from sysinfo Disks (may have limited data on Windows)
    // sysinfo doesn't provide I/O counters on Windows, so this is just a fallback
    
    // Method 3: Return 0 if nothing works
    // The metrics page will show 0, but at least it won't crash
    (0, 0)
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn get_disk_totals_from(_disks: &Disks) -> (u64, u64) {
    (0, 0)
}

/// Calculate I/O rates (bytes per second) from cumulative totals
fn calculate_io_rates() -> (u64, u64, u64, u64) {
    let mut state = IO_STATE.lock().unwrap();
    
    // Refresh networks and disks to get updated cumulative counters
    state.networks.refresh_list();
    state.disks.refresh_list();
    
    let (current_net_rx, current_net_tx) = get_network_totals_from(&state.networks);
    let (current_disk_read, current_disk_write) = get_disk_totals_from(&state.disks);
    
    let elapsed = state.last_update.elapsed().as_secs_f64();
    
    // Debug logging (log more frequently for troubleshooting)
    static mut LOG_COUNTER: u32 = 0;
    unsafe {
        LOG_COUNTER += 1;
        if LOG_COUNTER % 10 == 0 {
            debug!(
                "[Disk I/O] Current: read={} bytes, write={} bytes, Last: read={} bytes, write={} bytes, Elapsed: {:.2}s",
                current_disk_read, current_disk_write, state.last_disk_read, state.last_disk_write, elapsed
            );
        }
    }
    
    // Only update if at least 0.1 seconds have passed (more responsive)
    if elapsed >= 0.1 {
        // Calculate rates (handle counter resets gracefully)
        if current_net_rx >= state.last_net_rx {
            state.net_rx_rate = ((current_net_rx - state.last_net_rx) as f64 / elapsed) as u64;
        } else {
            // Counter reset or wrapped, keep previous rate temporarily
        }
        if current_net_tx >= state.last_net_tx {
            state.net_tx_rate = ((current_net_tx - state.last_net_tx) as f64 / elapsed) as u64;
        } else {
            // Counter reset or wrapped
        }
        
        // For disk I/O, handle the case where counters might start at 0
        // On first call or if counters reset, we need to initialize properly
        if state.last_disk_read == 0 && state.last_disk_write == 0 && current_disk_read == 0 && current_disk_write == 0 {
            // Both old and new values are 0 - likely iostat not working or no disk activity
            // Keep rates at 0
            unsafe {
                if LOG_COUNTER % 10 == 0 {
                    warn!("[Disk I/O] All values are 0 - iostat may not be working or no disk activity");
                }
            }
            state.disk_read_rate = 0;
            state.disk_write_rate = 0;
        } else if state.last_disk_read == 0 && state.last_disk_write == 0 {
            // First call with actual data - initialize counters but don't calculate rate yet
            // This prevents showing a huge spike on first measurement
            unsafe {
                if LOG_COUNTER % 10 == 0 {
                    debug!("[Disk I/O] First call with data - initializing counters: read={} bytes, write={} bytes", current_disk_read, current_disk_write);
                }
            }
            state.last_disk_read = current_disk_read;
            state.last_disk_write = current_disk_write;
            state.disk_read_rate = 0;
            state.disk_write_rate = 0;
        } else {
            // Calculate read rate
            if current_disk_read >= state.last_disk_read {
                let diff = current_disk_read - state.last_disk_read;
                state.disk_read_rate = (diff as f64 / elapsed) as u64;
                state.last_disk_read = current_disk_read;
                unsafe {
                    if LOG_COUNTER % 100 == 0 {
                        debug!("[Disk I/O] Read rate: {} bytes/sec (diff: {} bytes in {:.2}s)", state.disk_read_rate, diff, elapsed);
                    }
                }
            } else {
                // Counter reset or wrapped - keep previous rate but update counter
                warn!("[Disk I/O] Disk read counter decreased (possible reset): {} -> {}", state.last_disk_read, current_disk_read);
                state.last_disk_read = current_disk_read;
            }
            
            // Calculate write rate
            if current_disk_write >= state.last_disk_write {
                let diff = current_disk_write - state.last_disk_write;
                state.disk_write_rate = (diff as f64 / elapsed) as u64;
                state.last_disk_write = current_disk_write;
                unsafe {
                    if LOG_COUNTER % 100 == 0 {
                        debug!("[Disk I/O] Write rate: {} bytes/sec (diff: {} bytes in {:.2}s)", state.disk_write_rate, diff, elapsed);
                    }
                }
            } else {
                // Counter reset or wrapped - keep previous rate but update counter
                warn!("[Disk I/O] Disk write counter decreased (possible reset): {} -> {}", state.last_disk_write, current_disk_write);
                state.last_disk_write = current_disk_write;
            }
        }
        
        // Update last values and timestamp
        state.last_update = Instant::now();
        state.last_net_rx = current_net_rx;
        state.last_net_tx = current_net_tx;
    }
    
    (state.disk_read_rate, state.disk_write_rate, state.net_rx_rate, state.net_tx_rate)
}

/// Find processes related to AI model execution (optimized for periodic checks)
pub fn find_model_processes() -> Vec<ProcessInfo> {
    use sysinfo::ProcessesToUpdate;
    
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All); // Refresh all processes

    let model_keywords = ["ollama", "llama", "python", "torch", "cuda", "transformers"];
    let mut processes = Vec::new();

    for (pid, process) in sys.processes() {
        let name = process.name().to_string_lossy().to_lowercase();
        
        if model_keywords.iter().any(|k| name.contains(k)) {
            processes.push(ProcessInfo {
                pid: pid.as_u32(),
                name: process.name().to_string_lossy().to_string(),
                cpu_usage: process.cpu_usage(),
                memory_mb: process.memory() / (1024 * 1024),
                status: format!("{:?}", process.status()),
                start_time: process.start_time(),
            });
        }
    }

    processes
}

