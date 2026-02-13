// Rust PostCSS correctness harness — runs the combined first-3 plugin.
//
// Usage: verify_rust <input.css> <output.css>
//
// Applies the combined FirstThreePlugins (discardDuplicates + discardEmptyRules
// + parentOrphanedPseudos as a single pass) and writes the result.

use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: verify_rust <input.css> <output.css>");
        std::process::exit(1);
    }

    let css = fs::read_to_string(&args[1]).expect("Failed to read input CSS file");

    let mut proc = compiled_postcss::Processor::new(vec![
        Box::new(compiled_postcss::plugins::FirstThreePlugins),
    ]);
    let output = proc.process(&css);

    fs::write(&args[2], &output).expect("Failed to write output CSS file");
}
