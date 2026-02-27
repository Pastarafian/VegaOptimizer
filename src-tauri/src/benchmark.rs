//! System Benchmark — CPU, RAM, Disk speed tests

use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub cpu_single_score: f64,
    pub cpu_multi_score: f64,
    pub cpu_cores_used: usize,
    pub ram_read_mbps: f64,
    pub ram_write_mbps: f64,
    pub ram_latency_ns: f64,
    pub disk_seq_read_mbps: f64,
    pub disk_seq_write_mbps: f64,
    pub disk_random_iops: f64,
    pub total_score: u32,
    pub duration_ms: u64,
}

/// Run full system benchmark
pub fn run_benchmark() -> BenchmarkResult {
    let start = Instant::now();

    let cpu_single = bench_cpu_single();
    let cpu_multi = bench_cpu_multi();
    let (ram_read, ram_write, ram_lat) = bench_ram();
    let (disk_read, disk_write, disk_iops) = bench_disk();

    let cores = num_cpus();

    // Calculate composite score (weighted)
    let total = ((cpu_single * 0.15)
        + (cpu_multi * 0.15)
        + (ram_read / 10.0 * 0.1)
        + (ram_write / 10.0 * 0.1)
        + (disk_read / 5.0 * 0.2)
        + (disk_write / 5.0 * 0.15)
        + (disk_iops / 100.0 * 0.15))
        .min(10000.0) as u32;

    BenchmarkResult {
        cpu_single_score: cpu_single,
        cpu_multi_score: cpu_multi,
        cpu_cores_used: cores,
        ram_read_mbps: ram_read,
        ram_write_mbps: ram_write,
        ram_latency_ns: ram_lat,
        disk_seq_read_mbps: disk_read,
        disk_seq_write_mbps: disk_write,
        disk_random_iops: disk_iops,
        total_score: total,
        duration_ms: start.elapsed().as_millis() as u64,
    }
}

fn num_cpus() -> usize {
    sysinfo::System::physical_core_count().unwrap_or(4)
}

/// CPU single-core: tight math loop
fn bench_cpu_single() -> f64 {
    let start = Instant::now();
    let iterations = 5_000_000u64;
    let mut result = 0.0f64;

    for i in 0..iterations {
        result += (i as f64 * 0.000001).sin().cos().sqrt().abs();
    }

    let elapsed = start.elapsed().as_secs_f64();
    // Score: iterations per second normalized
    let ops_per_sec = iterations as f64 / elapsed;
    let _ = result; // Prevent optimization

    // Normalize to ~1000 for a modern single core
    (ops_per_sec / 5_000.0).min(5000.0)
}

/// CPU multi-core: parallel computation
fn bench_cpu_multi() -> f64 {
    let cores = num_cpus();
    let iterations_per_core = 3_000_000u64;
    let start = Instant::now();

    let handles: Vec<_> = (0..cores)
        .map(|core_id| {
            std::thread::spawn(move || {
                let mut result = 0.0f64;
                let offset = core_id as f64 * 0.1;
                for i in 0..iterations_per_core {
                    result += ((i as f64 + offset) * 0.000001).sin().cos().sqrt().abs();
                }
                result
            })
        })
        .collect();

    let _results: Vec<f64> = handles
        .into_iter()
        .map(|h| h.join().unwrap_or(0.0))
        .collect();
    let elapsed = start.elapsed().as_secs_f64();

    let total_ops = (cores as u64 * iterations_per_core) as f64;
    let ops_per_sec = total_ops / elapsed;

    (ops_per_sec / 5_000.0).min(20000.0)
}

/// RAM benchmark: sequential read/write speed
fn bench_ram() -> (f64, f64, f64) {
    let size = 64 * 1024 * 1024; // 64 MB
    let iterations = 4;

    // Write test
    let start = Instant::now();
    for _ in 0..iterations {
        let mut buf = vec![0u8; size];
        for chunk in buf.chunks_mut(8) {
            if chunk.len() == 8 {
                chunk.copy_from_slice(&0x5555555555555555u64.to_le_bytes());
            }
        }
        std::hint::black_box(&buf);
    }
    let write_elapsed = start.elapsed().as_secs_f64();
    let write_mbps = (size as f64 * iterations as f64 / 1_048_576.0) / write_elapsed;

    // Read test
    let buf = vec![42u8; size];
    let start = Instant::now();
    let mut sum = 0u64;
    for _ in 0..iterations {
        for chunk in buf.chunks(8) {
            if chunk.len() == 8 {
                sum = sum.wrapping_add(u64::from_le_bytes(chunk.try_into().unwrap_or([0; 8])));
            }
        }
    }
    let read_elapsed = start.elapsed().as_secs_f64();
    let read_mbps = (size as f64 * iterations as f64 / 1_048_576.0) / read_elapsed;
    let _ = sum;

    // Latency test (random access pattern)
    let lat_buf = vec![0u64; 1024 * 1024];
    let start = Instant::now();
    let mut idx = 0usize;
    for _ in 0..1_000_000 {
        idx = (idx.wrapping_mul(6364136223846793005).wrapping_add(1)) % lat_buf.len();
        std::hint::black_box(lat_buf[idx]);
    }
    let lat_elapsed = start.elapsed().as_nanos() as f64;
    let lat_ns = lat_elapsed / 1_000_000.0;

    (read_mbps, write_mbps, lat_ns)
}

/// Disk benchmark: sequential + random I/O
fn bench_disk() -> (f64, f64, f64) {
    let temp = std::env::var("TEMP").unwrap_or_else(|_| ".".into());
    let path = format!("{}\\vega_bench_{}.tmp", temp, std::process::id());
    let block_size = 1024 * 1024; // 1 MB blocks
    let blocks = 64; // 64 MB total

    // Sequential write
    let data: Vec<u8> = (0..block_size).map(|i| (i % 256) as u8).collect();
    let start = Instant::now();
    if let Ok(mut f) = std::fs::File::create(&path) {
        use std::io::Write;
        for _ in 0..blocks {
            let _ = f.write_all(&data);
        }
        let _ = f.sync_all();
    }
    let write_elapsed = start.elapsed().as_secs_f64();
    let write_mbps = (blocks as f64 * block_size as f64 / 1_048_576.0) / write_elapsed;

    // Sequential read
    let start = Instant::now();
    if let Ok(content) = std::fs::read(&path) {
        std::hint::black_box(&content);
    }
    let read_elapsed = start.elapsed().as_secs_f64();
    let read_mbps = (blocks as f64 * block_size as f64 / 1_048_576.0) / read_elapsed;

    // Random read (4K blocks)
    let small_block = 4096;
    let random_reads = 1000;
    let file_size = blocks * block_size;
    let start = Instant::now();
    if let Ok(f) = std::fs::File::open(&path) {
        use std::io::{Read, Seek, SeekFrom};
        let mut f = std::io::BufReader::new(f);
        let mut buf = vec![0u8; small_block];
        let mut offset = 0u64;
        for i in 0..random_reads {
            offset = (offset.wrapping_mul(6364136223846793005).wrapping_add(i))
                % (file_size as u64 - small_block as u64);
            let _ = f.seek(SeekFrom::Start(offset));
            let _ = f.read_exact(&mut buf);
        }
    }
    let random_elapsed = start.elapsed().as_secs_f64();
    let iops = random_reads as f64 / random_elapsed;

    // Cleanup
    let _ = std::fs::remove_file(&path);

    (read_mbps, write_mbps, iops)
}
