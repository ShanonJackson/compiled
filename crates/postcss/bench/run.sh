#!/usr/bin/env bash
# PostCSS Rust vs JS benchmark + correctness verification
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CRATE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
FIXTURES_DIR="$SCRIPT_DIR/fixtures"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m' # No Color

echo ""
echo -e "${BOLD}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BOLD}║      PostCSS Benchmark: Rust vs JavaScript (8.4.31)        ║${NC}"
echo -e "${BOLD}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# ── Step 1: Generate fixtures if needed ─────────────────────────────────
if [ ! -f "$FIXTURES_DIR/small.css" ]; then
    echo -e "${CYAN}Generating fixtures...${NC}"
    node "$SCRIPT_DIR/generate_fixtures.js"
    echo ""
fi

# ── Step 2: Build Rust binary (release mode) ────────────────────────────
echo -e "${CYAN}Building Rust binary (release mode)...${NC}"
cd "$CRATE_DIR"
cargo build --release --bin bench_rust 2>&1 | grep -v "^$" | head -5
RUST_BIN="$CRATE_DIR/target/release/bench_rust"
echo -e "${GREEN}Done.${NC}"
echo ""

# ── Step 3: Run benchmarks ──────────────────────────────────────────────
FIXTURES=("small.css" "medium.css" "large.css")
ITERS=(5000 1000 100)

echo -e "${BOLD}┌─────────────┬──────────┬─────────────────┬─────────────────┬────────────┐${NC}"
echo -e "${BOLD}│ Fixture     │ Size     │   JS (avg/iter) │ Rust (avg/iter) │   Speedup  │${NC}"
echo -e "${BOLD}├─────────────┼──────────┼─────────────────┼─────────────────┼────────────┤${NC}"

for idx in "${!FIXTURES[@]}"; do
    fixture="${FIXTURES[$idx]}"
    iters="${ITERS[$idx]}"
    css_file="$FIXTURES_DIR/$fixture"

    # Run JS benchmark
    js_json=$(node "$SCRIPT_DIR/bench_js.js" "$css_file" "$iters")
    js_avg=$(echo "$js_json" | node -e "process.stdin.on('data',d=>{const j=JSON.parse(d);console.log(j.avg_us)})")
    js_ops=$(echo "$js_json" | node -e "process.stdin.on('data',d=>{const j=JSON.parse(d);console.log(j.ops_per_sec)})")

    # Run Rust benchmark
    rs_json=$("$RUST_BIN" "$css_file" "$iters")
    rs_avg=$(echo "$rs_json" | node -e "process.stdin.on('data',d=>{const j=JSON.parse(d);console.log(j.avg_us)})")
    rs_ops=$(echo "$rs_json" | node -e "process.stdin.on('data',d=>{const j=JSON.parse(d);console.log(j.ops_per_sec)})")

    # Compute speedup
    speedup=$(node -e "console.log((${js_avg}/${rs_avg}).toFixed(1))")

    # Format size
    size_kb=$(node -e "const s=require('fs').statSync('$css_file').size;console.log((s/1024).toFixed(1)+'KB')")

    # Format times
    if [ "$js_avg" -ge 1000 ]; then
        js_disp="$(node -e "console.log((${js_avg}/1000).toFixed(2))")ms"
    else
        js_disp="${js_avg}us"
    fi
    if [ "$rs_avg" -ge 1000 ]; then
        rs_disp="$(node -e "console.log((${rs_avg}/1000).toFixed(2))")ms"
    else
        rs_disp="${rs_avg}us"
    fi

    printf "${BOLD}│${NC} %-11s ${BOLD}│${NC} %8s ${BOLD}│${NC} %15s ${BOLD}│${NC} %15s ${BOLD}│${NC} ${GREEN}%9sx${NC} ${BOLD}│${NC}\n" \
        "$fixture" "$size_kb" "$js_disp" "$rs_disp" "$speedup"
done

echo -e "${BOLD}└─────────────┴──────────┴─────────────────┴─────────────────┴────────────┘${NC}"
echo ""

# ── Step 4: Correctness verification ────────────────────────────────────
echo -e "${BOLD}Correctness Verification (byte-identical output):${NC}"
echo ""

all_pass=true
for fixture in "${FIXTURES[@]}"; do
    css_file="$FIXTURES_DIR/$fixture"
    js_out="${css_file%.css}.js_output.css"
    rs_out="${css_file%.css}.rs_output.css"

    if [ ! -f "$js_out" ] || [ ! -f "$rs_out" ]; then
        echo -e "  ${YELLOW}SKIP${NC} $fixture — output files not found"
        continue
    fi

    # First check: does roundtrip preserve the input?
    if diff -q "$css_file" "$js_out" > /dev/null 2>&1; then
        js_roundtrip="${GREEN}roundtrip OK${NC}"
    else
        js_roundtrip="${YELLOW}roundtrip differs${NC}"
    fi

    if diff -q "$css_file" "$rs_out" > /dev/null 2>&1; then
        rs_roundtrip="${GREEN}roundtrip OK${NC}"
    else
        rs_roundtrip="${YELLOW}roundtrip differs${NC}"
    fi

    # Critical check: does Rust output match JS output?
    if diff -q "$js_out" "$rs_out" > /dev/null 2>&1; then
        echo -e "  ${GREEN}PASS${NC} $fixture — Rust output is byte-identical to JS output"
    else
        echo -e "  ${RED}FAIL${NC} $fixture — outputs differ!"
        all_pass=false
        # Show first diff
        echo -e "  ${YELLOW}First difference:${NC}"
        diff --unified=3 "$js_out" "$rs_out" | head -20
        echo ""
        # Show byte comparison
        js_size=$(wc -c < "$js_out")
        rs_size=$(wc -c < "$rs_out")
        echo -e "  JS output: ${js_size} bytes"
        echo -e "  RS output: ${rs_size} bytes"
    fi
    echo -e "     JS: ${js_roundtrip}  |  Rust: ${rs_roundtrip}"
done

echo ""
if $all_pass; then
    echo -e "${GREEN}${BOLD}All correctness checks passed!${NC}"
else
    echo -e "${RED}${BOLD}Some correctness checks failed — see diffs above.${NC}"
fi
echo ""
