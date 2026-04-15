//! Scan the Dorfromantik game process memory for the camera position.
//! Uses /proc/<pid>/mem to read process memory.
//!
//! Strategy:
//! 1. Take a snapshot of all f32 values in writable memory
//! 2. User pans camera, take another snapshot
//! 3. Filter for values that changed
//! 4. User stops, take another snapshot
//! 5. Filter for values that stayed the same
//! 6. Repeat until narrowed down
//!
//! Run with: sudo cargo run --example scan_camera

use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, Read, Seek, SeekFrom};

struct Region {
    start: u64,
    end: u64,
}

fn get_writable_regions(pid: u32) -> Vec<Region> {
    let maps = fs::read_to_string(format!("/proc/{pid}/maps")).unwrap();
    let mut regions = Vec::new();
    for line in maps.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let perms = parts[1];
        if !perms.contains("rw") {
            continue;
        }
        let addrs: Vec<&str> = parts[0].split('-').collect();
        let start = u64::from_str_radix(addrs[0], 16).unwrap();
        let end = u64::from_str_radix(addrs[1], 16).unwrap();
        let size = end - start;
        // Skip tiny regions and large ones (>16MB)
        if !(4096..=16 * 1024 * 1024).contains(&size) {
            continue;
        }
        regions.push(Region { start, end });
    }
    regions
}

fn snapshot_floats(pid: u32, regions: &[Region]) -> HashMap<u64, f32> {
    let mut file = fs::File::open(format!("/proc/{pid}/mem")).unwrap();
    let mut values = HashMap::new();

    for region in regions {
        let size = (region.end - region.start) as usize;
        let mut buf = vec![0u8; size];
        if file.seek(SeekFrom::Start(region.start)).is_err() {
            continue;
        }
        if file.read_exact(&mut buf).is_err() {
            continue;
        }

        // Scan for f32 values at 4-byte aligned positions
        for offset in (0..size - 3).step_by(4) {
            let val = f32::from_le_bytes([
                buf[offset],
                buf[offset + 1],
                buf[offset + 2],
                buf[offset + 3],
            ]);
            // Filter: plausible world coordinates (-10000 to 10000), not NaN/Inf, not zero
            if val.is_finite() && val.abs() > 0.01 && val.abs() < 10000.0 {
                values.insert(region.start + offset as u64, val);
            }
        }
    }
    values
}

fn read_float(pid: u32, addr: u64) -> Option<f32> {
    let mut file = fs::File::open(format!("/proc/{pid}/mem")).ok()?;
    let mut buf = [0u8; 4];
    file.seek(SeekFrom::Start(addr)).ok()?;
    file.read_exact(&mut buf).ok()?;
    Some(f32::from_le_bytes(buf))
}

fn main() {
    let pid: u32 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            // Auto-detect
            let output = std::process::Command::new("pgrep")
                .args(["-f", "Z:.*Dorfromantik.exe"])
                .output()
                .unwrap();
            let s = String::from_utf8_lossy(&output.stdout);
            s.lines().next().unwrap().trim().parse().unwrap()
        });

    println!("Scanning PID {pid}");
    let regions = get_writable_regions(pid);
    let total_bytes: u64 = regions.iter().map(|r| r.end - r.start).sum();
    println!(
        "{} writable regions, {:.0} MB total",
        regions.len(),
        total_bytes as f64 / 1e6
    );

    let stdin = io::stdin();
    let mut candidates: HashMap<u64, f32> = HashMap::new();
    let mut step = 0;

    loop {
        println!("\n--- Step {} ---", step);
        if step == 0 {
            println!("Don't move the camera. Press Enter to take initial snapshot...");
        } else {
            println!("Commands:");
            println!("  s  = snapshot stable (keep only unchanged values)");
            println!("  c  = snapshot changed (keep only changed values)");
            println!("  l  = list current candidates");
            println!("  q  = quit");
        }

        let mut input = String::new();
        stdin.lock().read_line(&mut input).unwrap();
        let cmd = input.trim();

        if step == 0 {
            println!("Taking initial snapshot...");
            candidates = snapshot_floats(pid, &regions);
            println!("{} candidate floats", candidates.len());
            step += 1;
            continue;
        }

        match cmd {
            "s" => {
                println!("Filtering for stable values...");
                let mut kept = 0;
                let mut removed = 0;
                let mut to_remove = Vec::new();
                for (&addr, &old_val) in &candidates {
                    match read_float(pid, addr) {
                        Some(new_val) if (new_val - old_val).abs() < 0.001 => {
                            kept += 1;
                        }
                        _ => {
                            to_remove.push(addr);
                            removed += 1;
                        }
                    }
                }
                for addr in to_remove {
                    candidates.remove(&addr);
                }
                println!(
                    "Kept {kept}, removed {removed}, remaining: {}",
                    candidates.len()
                );
            }
            "c" => {
                println!("Filtering for changed values...");
                let mut new_candidates = HashMap::new();
                for (&addr, &old_val) in &candidates {
                    match read_float(pid, addr) {
                        Some(new_val) if (new_val - old_val).abs() > 0.01 => {
                            new_candidates.insert(addr, new_val);
                        }
                        _ => {}
                    }
                }
                println!(
                    "Changed: {}, was: {}",
                    new_candidates.len(),
                    candidates.len()
                );
                candidates = new_candidates;
            }
            "l" => {
                let mut sorted: Vec<_> = candidates.iter().collect();
                sorted.sort_by_key(|(addr, _)| *addr);
                for (i, (&addr, &val)) in sorted.iter().enumerate().take(100) {
                    // Also read the next 2 floats (potential y,z of Vector3)
                    let y = read_float(pid, addr + 4).unwrap_or(0.0);
                    let z = read_float(pid, addr + 8).unwrap_or(0.0);
                    println!("  {i:3}. 0x{addr:012x}: {val:12.4} ({y:12.4}, {z:12.4})");
                }
                if sorted.len() > 100 {
                    println!("  ... and {} more", sorted.len() - 100);
                }
            }
            "q" => break,
            _ => println!("Unknown command: {cmd}"),
        }
        step += 1;
    }
}
