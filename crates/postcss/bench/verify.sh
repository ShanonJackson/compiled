#!/usr/bin/env bash
# PostCSS First-3 Plugins: Rust vs JS correctness verification.
#
# Runs the same CSS inputs through both:
#   JS:   PostCSS 8.4.31 with discardDuplicates + discardEmptyRules + parentOrphanedPseudos
#   Rust: compiled-postcss with combined FirstThreePlugins (single-pass)
#
# Asserts byte-identical output for every test case.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CRATE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

echo ""
echo -e "${BOLD}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BOLD}║   First-3 Plugins: Rust vs JS Correctness Verification     ║${NC}"
echo -e "${BOLD}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# ── Build Rust binary ───────────────────────────────────────────────────────
echo -e "${CYAN}Building Rust verify binary (release)...${NC}"
cd "$CRATE_DIR"
cargo build --release --bin verify_rust 2>&1 | grep -v "^$" | grep -v "^warning" | head -5
RUST_BIN="$CRATE_DIR/target/release/verify_rust"
echo -e "${GREEN}Done.${NC}"
echo ""

# ── Setup temp dir ──────────────────────────────────────────────────────────
TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

pass=0
fail=0
total=0

run_test() {
    local name="$1"
    local css="$2"
    total=$((total + 1))

    local input="$TMPDIR/input_${total}.css"
    local js_out="$TMPDIR/js_${total}.css"
    local rs_out="$TMPDIR/rs_${total}.css"

    printf '%s' "$css" > "$input"

    # Run JS
    node "$SCRIPT_DIR/verify_js.js" "$input" "$js_out" 2>/dev/null

    # Run Rust
    "$RUST_BIN" "$input" "$rs_out" 2>/dev/null

    # Compare
    if diff -q "$js_out" "$rs_out" > /dev/null 2>&1; then
        echo -e "  ${GREEN}PASS${NC}  $name"
        pass=$((pass + 1))
    else
        echo -e "  ${RED}FAIL${NC}  $name"
        fail=$((fail + 1))
        echo -e "  ${YELLOW}--- JS output:${NC}"
        cat "$js_out" | head -10
        echo -e "  ${YELLOW}--- Rust output:${NC}"
        cat "$rs_out" | head -10
        echo -e "  ${YELLOW}--- Diff:${NC}"
        diff --unified=3 "$js_out" "$rs_out" | head -20
        echo ""
    fi
}

# ═══════════════════════════════════════════════════════════════════════════
# TEST CORPUS
# ═══════════════════════════════════════════════════════════════════════════

echo -e "${BOLD}Category: discardDuplicates${NC}"

run_test "no duplicates" \
    '
      display: block;
      margin: 0 auto;
    '

run_test "simple duplicate" \
    '
      display: block;
      display: flex;
    '

run_test "multiple duplicate props" \
    '
      color: red;
      display: block;
      color: blue;
      display: flex;
    '

run_test "three of same prop" \
    '
      color: red;
      color: green;
      color: blue;
    '

run_test "duplicates with !important" \
    '
      color: red !important;
      color: blue;
    '

echo ""
echo -e "${BOLD}Category: discardEmptyRules${NC}"

run_test "undefined value" \
    '
      display: undefined;
      color: red;
    '

run_test "null value" \
    '
      display: null;
      color: red;
    '

run_test "empty value" \
    '
      display: ;
      color: red;
    '

run_test "whitespace-only value" \
    '
      display:    ;
      color: red;
    '

run_test "empty value inside rule" \
    '
      :hover {
        display: undefined;
        color: red;
      }
    '

run_test "rule removed when all empty" \
    '
      .class {
        display: undefined;
      }
    '

run_test "multiple empty values in rule" \
    '
      .class {
        display: undefined;
        color: null;
        background: ;
      }
    '

run_test "empty value does not remove non-empty siblings" \
    '
      display: undefined;
      color: red;
      font-size: 14px;
    '

echo ""
echo -e "${BOLD}Category: parentOrphanedPseudos${NC}"

run_test "orphaned :hover" \
    '
      :hover {
        display: block;
      }
    '

run_test "orphaned ::before" \
    '
      ::before {
        content: "";
      }
    '

run_test "orphaned :first-child with &" \
    '
      :first-child & {
        color: hotpink;
      }
    '

run_test "multiple pseudo groups :hover, :active" \
    '
      :hover, :active {
        display: block;
      }
    '

run_test "mixed groups div, :active" \
    '
      div, :active {
        display: block;
      }
    '

run_test "no-op &:hover" \
    '
      div {
        &:hover {
          display: block;
        }
      }
    '

run_test "no-op combinator before pseudo" \
    '
      div {
        div > :hover {
          display: block;
        }
      }
    '

run_test "no-op attribute selector with &" \
    '
      [data-look='"'"'h100'"'"']& {
        display: block;
      }
    '

run_test "nested orphaned pseudo" \
    '
      div {
        :hover {
          display: block;
        }
      }
    '

run_test "orphaned :nth-child(2n+1)" \
    '
      :nth-child(2n+1) {
        color: red;
      }
    '

run_test "orphaned :not(.foo)" \
    '
      :not(.foo) {
        color: red;
      }
    '

echo ""
echo -e "${BOLD}Category: Combined interactions${NC}"

run_test "dedup + empty: color:red then color:" \
    '
      color: red;
      color: ;
    '

run_test "empty inside orphaned pseudo rule" \
    '
      :hover {
        display: undefined;
      }
    '

run_test "all three together" \
    '
      display: block;
      display: flex;
      :hover {
        color: null;
        background: red;
      }
    '

run_test "dedup + pseudo in same stylesheet" \
    '
      margin: 0;
      margin: 10px;
      :focus {
        outline: none;
      }
    '

run_test "empty removes rule, pseudo transforms sibling" \
    '
      .empty {
        display: undefined;
      }
      :active {
        color: blue;
      }
    '

echo ""
echo -e "${BOLD}Category: Edge cases${NC}"

run_test "empty input" ""

run_test "whitespace only" \
    '
    '

run_test "comment only" \
    '/* hello */'

run_test "comment between declarations" \
    '
      color: red;
      /* comment */
      color: blue;
    '

run_test "@media with orphaned pseudo inside" \
    '
      @media (min-width: 768px) {
        :hover {
          color: red;
        }
      }
    '

run_test "@media with empty rule inside" \
    '
      @media (min-width: 768px) {
        .class {
          display: undefined;
        }
      }
    '

run_test "deeply nested pseudo" \
    '
      @media screen {
        div {
          :hover {
            color: red;
          }
        }
      }
    '

run_test "rule with !important survives" \
    '
      color: red !important;
    '

run_test "multiple selectors with newline separator" \
    ':hover,
:active {
  display: block;
}'

run_test "complex mixed stylesheet" \
    '
      /* Global styles */
      font-size: 16px;
      font-size: 14px;

      .component {
        display: undefined;
      }

      :hover {
        color: red;
      }

      :focus, :active {
        outline: none;
      }

      @media (max-width: 600px) {
        :hover {
          background: null;
          color: blue;
        }
      }

      border: ;
      margin: 10px;
    '

# ═══════════════════════════════════════════════════════════════════════════
# RESULTS
# ═══════════════════════════════════════════════════════════════════════════

echo ""
echo -e "${BOLD}────────────────────────────────────────────${NC}"
echo -e "  Total: ${total}  ${GREEN}Pass: ${pass}${NC}  ${RED}Fail: ${fail}${NC}"
echo -e "${BOLD}────────────────────────────────────────────${NC}"
echo ""

if [ "$fail" -eq 0 ]; then
    echo -e "${GREEN}${BOLD}All correctness checks passed!${NC}"
    echo -e "Rust single-pass plugin output is byte-identical to JS 3-plugin sequential output."
    exit 0
else
    echo -e "${RED}${BOLD}${fail} correctness check(s) failed!${NC}"
    exit 1
fi
