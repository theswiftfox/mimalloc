//! Allocator micro/macro benchmarks for tuning the `mimalloc` crate.
//!
//! Usage: `mimalloc-bench [workload ...]` (default: all). Set `BENCH_THREADS` (default 4)
//! and `BENCH_SCALE` (default 1) to change the load.
use std::collections::HashMap;
use std::hint::black_box;
use std::mem::MaybeUninit;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(all(feature = "mimalloc", not(feature = "system")))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    // mostly small sizes with a long tail, like typical Rust programs
    fn size(&mut self) -> usize {
        match self.below(100) {
            0..=69 => 8 + self.below(120),
            70..=94 => 128 + self.below(896),
            95..=98 => 1024 + self.below(15 * 1024),
            _ => 16 * 1024 + self.below(240 * 1024),
        }
    }
}

fn uninit(n: usize) -> Box<[MaybeUninit<u8>]> {
    let mut b = Box::<[u8]>::new_uninit_slice(n);
    // touch the block like a real program would
    b[0] = MaybeUninit::new(1);
    b
}

fn scale() -> usize {
    std::env::var("BENCH_SCALE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1)
}

fn threads() -> usize {
    std::env::var("BENCH_THREADS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4)
}

// CPU workloads; each returns the number of operations.

// pure fast path: allocate and immediately free a 32-byte box
fn fixed_32b() -> u64 {
    let n = 50_000_000 * scale();
    for i in 0..n {
        let b = Box::new([i as u64; 4]);
        black_box(&b);
    }
    n as u64
}

// random replacement in a working set of 4096 live objects of mixed sizes
fn churn_st() -> u64 {
    churn(1, 20_000_000 * scale())
}

fn churn(seed: u64, n: usize) -> u64 {
    let mut rng = Rng::new(seed);
    let mut live: Vec<Option<Box<[MaybeUninit<u8>]>>> = (0..4096).map(|_| None).collect();
    for _ in 0..n {
        let i = rng.below(live.len());
        live[i] = Some(uninit(rng.size()));
    }
    black_box(&live);
    n as u64
}

fn churn_mt() -> u64 {
    let t = threads();
    let n = 10_000_000 * scale();
    let hs: Vec<_> = (0..t)
        .map(|i| thread::spawn(move || churn(i as u64 + 7, n)))
        .collect();
    hs.into_iter().map(|h| h.join().unwrap()).sum()
}

// zeroed allocations (e.g. `vec![0; n]`)
fn zeroed() -> u64 {
    let mut rng = Rng::new(3);
    let n = 10_000_000 * scale();
    for _ in 0..n {
        let v = vec![0u8; 8 + rng.below(1016)];
        black_box(&v);
    }
    n as u64
}

// growing vectors (realloc path)
fn vec_grow() -> u64 {
    let mut rng = Rng::new(5);
    let mut ops = 0;
    for _ in 0..200_000 * scale() {
        let len = 1 + rng.below(4096);
        let mut v: Vec<u32> = Vec::new();
        for j in 0..len {
            v.push(j as u32);
        }
        black_box(&v);
        ops += len as u64;
    }
    ops
}

// as `vec_grow` but with the final capacity up front (control: no reallocs)
fn vec_cap() -> u64 {
    let mut rng = Rng::new(5);
    let mut ops = 0;
    for _ in 0..200_000 * scale() {
        let len = 1 + rng.below(4096);
        let mut v: Vec<u32> = Vec::with_capacity(len);
        for j in 0..len {
            v.push(j as u32);
        }
        black_box(&v);
        ops += len as u64;
    }
    ops
}

// string-heavy map building
fn strings() -> u64 {
    let mut ops = 0;
    for round in 0..20 * scale() {
        let mut m: HashMap<String, Vec<String>> = HashMap::new();
        for i in 0..50_000 {
            let k = format!("key-{round}-{}", i % 10_000);
            m.entry(k).or_default().push(format!("value-{i}"));
            ops += 1;
        }
        black_box(&m);
    }
    ops
}

// producers allocate, a consumer thread frees (cross-thread frees)
fn xthread() -> u64 {
    let producers = threads().max(2) - 1;
    let batches = 20_000 * scale();
    let (tx, rx) = mpsc::sync_channel::<Vec<Box<[MaybeUninit<u8>]>>>(64);
    let consumer = thread::spawn(move || {
        let mut n = 0u64;
        for batch in rx {
            n += batch.len() as u64;
            drop(batch);
        }
        n
    });
    let hs: Vec<_> = (0..producers)
        .map(|p| {
            let tx = tx.clone();
            thread::spawn(move || {
                let mut rng = Rng::new(p as u64 + 11);
                for _ in 0..batches {
                    let batch: Vec<_> = (0..256).map(|_| uninit(16 + rng.below(240))).collect();
                    tx.send(batch).unwrap();
                }
            })
        })
        .collect();
    drop(tx);
    for h in hs {
        h.join().unwrap();
    }
    consumer.join().unwrap()
}

// Memory workloads.

fn status_kib(field: &str) -> u64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    s.lines()
        .find(|l| l.starts_with(field))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

fn rss_mib() -> f64 {
    status_kib("VmRSS:") as f64 / 1024.0
}

// a service-like pattern: threads build a large working set, release most of it, and keep running
fn memory() {
    let t = threads();
    let per_thread = 64 * 1024 * 1024 * scale();
    let base = rss_mib();
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let (go_tx, go_rx) = mpsc::channel::<()>();
    let go_rx = std::sync::Arc::new(std::sync::Mutex::new(go_rx));
    let start = Instant::now();
    let hs: Vec<_> = (0..t)
        .map(|i| {
            let done_tx = done_tx.clone();
            let go_rx = go_rx.clone();
            thread::spawn(move || {
                let mut rng = Rng::new(i as u64 + 101);
                let mut live = Vec::new();
                let mut bytes = 0;
                while bytes < per_thread {
                    let n = rng.size();
                    let mut b = uninit(n);
                    for j in (0..n).step_by(4096) {
                        b[j] = MaybeUninit::new(1);
                    }
                    bytes += n;
                    live.push(b);
                }
                done_tx.send(()).unwrap();
                go_rx.lock().unwrap().recv().unwrap();
                // keep a random 10% and keep running with a small working set so purging can happen
                live.retain(|_| rng.below(10) == 0);
                let deadline = Instant::now() + Duration::from_millis(5000);
                while Instant::now() < deadline {
                    for _ in 0..1_000 {
                        let i = rng.below(live.len());
                        live[i] = uninit(8 + rng.below(500));
                    }
                    thread::sleep(Duration::from_millis(1));
                }
                black_box(&live);
            })
        })
        .collect();
    for _ in 0..t {
        done_rx.recv().unwrap();
    }
    let build = start.elapsed();
    let peak = rss_mib();
    for _ in 0..t {
        go_tx.send(()).unwrap();
    }
    let freed = Instant::now();
    let mut timeline = String::new();
    for ms in [20u64, 100, 500, 1000, 2000, 4500] {
        thread::sleep(Duration::from_millis(ms).saturating_sub(freed.elapsed()));
        timeline += &format!(" {ms}ms {:>6.1}", rss_mib());
    }
    for h in hs {
        h.join().unwrap();
    }
    println!(
        "{:<10} build {:>6.1} ms | rss MiB: base {base:>5.1} peak {peak:>6.1} | after free:{timeline}",
        "memory",
        build.as_secs_f64() * 1e3
    );
}

fn minor_faults() -> u64 {
    let s = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    // the fields after the command name (which is in parentheses)
    s.rsplit(')')
        .next()
        .and_then(|r| r.split_whitespace().nth(7))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

// request bursts with idle gaps in between: short purge delays return memory in the gaps
// but have to fault it back in for the next burst
fn burst() {
    let t = threads();
    let faults = minor_faults();
    let hs: Vec<_> = (0..t)
        .map(|i| {
            thread::spawn(move || {
                let mut rng = Rng::new(i as u64 + 77);
                let mut active = Duration::ZERO;
                for _ in 0..40 * scale() {
                    let start = Instant::now();
                    let mut live = Vec::new();
                    let mut bytes = 0;
                    while bytes < 16 * 1024 * 1024 {
                        let n = rng.size();
                        let mut b = uninit(n);
                        for j in (0..n).step_by(4096) {
                            b[j] = MaybeUninit::new(1);
                        }
                        bytes += n;
                        live.push(b);
                    }
                    drop(live);
                    active += start.elapsed();
                    thread::sleep(Duration::from_millis(30));
                }
                active
            })
        })
        .collect();
    let active: Duration = hs.into_iter().map(|h| h.join().unwrap()).sum();
    println!(
        "{:<10} {:>8.1} ms active | {} minor faults",
        "burst",
        active.as_secs_f64() * 1e3 / t as f64,
        minor_faults() - faults
    );
}

// many threads with small heaps (per-thread page overhead dominates)
fn threads_mem() {
    let t = 64;
    let base = rss_mib();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(t + 1));
    let hs: Vec<_> = (0..t)
        .map(|i| {
            let barrier = barrier.clone();
            thread::spawn(move || {
                let mut rng = Rng::new(i as u64 + 1001);
                let mut live = Vec::new();
                let mut bytes = 0;
                for _ in 0..400 {
                    let n = 8 + rng.below(2040);
                    let mut b = uninit(n);
                    for j in (0..n).step_by(512) {
                        b[j] = MaybeUninit::new(1);
                    }
                    bytes += n;
                    live.push(b);
                }
                barrier.wait();
                barrier.wait();
                black_box(&live);
                bytes
            })
        })
        .collect();
    barrier.wait();
    let rss = rss_mib();
    barrier.wait();
    let live: usize = hs.into_iter().map(|h| h.join().unwrap()).sum();
    println!(
        "{:<10} {t} threads, live {:>6.1} MiB | rss MiB: {:>6.1} (overhead {:>6.1})",
        "thrd_mem",
        live as f64 / (1024.0 * 1024.0),
        rss - base,
        rss - base - live as f64 / (1024.0 * 1024.0)
    );
}

fn main() {
    let workloads: [(&str, fn() -> u64); 8] = [
        ("fixed_32b", fixed_32b),
        ("churn_st", churn_st),
        ("churn_mt", churn_mt),
        ("zeroed", zeroed),
        ("vec_grow", vec_grow),
        ("vec_cap", vec_cap),
        ("strings", strings),
        ("xthread", xthread),
    ];
    let args: Vec<String> = std::env::args().skip(1).collect();
    let selected = |name: &str| args.is_empty() || args.iter().any(|a| a == name);
    for (name, f) in workloads {
        if selected(name) {
            let start = Instant::now();
            let ops = f();
            let secs = start.elapsed().as_secs_f64();
            println!(
                "{name:<10} {:>8.1} ms {:>8.1} Mops/s",
                secs * 1e3,
                ops as f64 / secs / 1e6
            );
        }
    }
    if selected("memory") {
        memory();
    }
    if selected("thrd_mem") {
        threads_mem();
    }
    if selected("burst") {
        burst();
    }
}
