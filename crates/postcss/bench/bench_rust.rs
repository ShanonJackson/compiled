// Rust PostCSS benchmark — parse → stringify roundtrip
// Outputs JSON: { file, size_kb, iterations, total_ms, avg_us, ops_per_sec, output_hash }

use std::env;
use std::fs;
use std::time::Instant;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: bench_rust <css-file> [iterations]");
        std::process::exit(1);
    }

    let file = &args[1];
    let iterations: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(100);
    let css = fs::read_to_string(file).expect("Failed to read CSS file");
    let size_kb = css.len() as f64 / 1024.0;

    // Warmup (5 iterations).
    for _ in 0..5 {
        let ss = compiled_postcss::parse(&css);
        let _ = compiled_postcss::stringify(&ss);
    }

    // Benchmark.
    let mut output = String::new();
    let start = Instant::now();
    for _ in 0..iterations {
        let ss = compiled_postcss::parse(&css);
        output = compiled_postcss::stringify(&ss);
    }
    let elapsed = start.elapsed();

    let total_ms = elapsed.as_millis();
    let avg_us = elapsed.as_micros() / iterations as u128;
    let ops_per_sec = if total_ms > 0 {
        (iterations as u64 * 1000) / total_ms as u64
    } else {
        iterations as u64 * 1_000_000 // sub-millisecond
    };

    // Simple hash for comparison (sum of bytes mod 2^32, base16).
    let hash = simple_hash(&output);

    println!(
        r#"{{"file":"{}","size_kb":{:.1},"iterations":{},"total_ms":{},"avg_us":{},"ops_per_sec":{},"output_hash":"{}"}}"#,
        file.rsplit('/').next().unwrap_or(file),
        size_kb,
        iterations,
        total_ms,
        avg_us,
        ops_per_sec,
        hash
    );

    // Write output for correctness comparison.
    let out_file = file.replace(".css", ".rs_output.css");
    fs::write(&out_file, &output).expect("Failed to write output");
}

fn simple_hash(s: &str) -> String {
    // Use the same approach: we'll do a simple FNV-style hash and take 12 hex chars.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:012x}", h & 0xffffffffffff)
}
