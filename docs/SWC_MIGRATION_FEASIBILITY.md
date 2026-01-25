# SWC Migration Feasibility Analysis

## Executive Summary

**Goal:** Create native SWC (Rust) equivalents of `@compiled/babel-plugin` that produce **100% byte-identical output** in every case.

**Overall Feasibility:** ✅ **Achievable but Highly Complex**

**Confidence Level:** 75-85% (with caveats)

**Estimated Scope:** ~15,000-25,000 lines of Rust code

---

## Risk Assessment Matrix

| Component             | Complexity | Risk Level | Confidence | Notes                                      |
| --------------------- | ---------- | ---------- | ---------- | ------------------------------------------ |
| MurmurHash2           | Low        | 🟢 Low     | 99%        | Well-documented algorithm, easy to port    |
| AST Traversal         | Medium     | 🟢 Low     | 95%        | SWC has excellent visitor patterns         |
| Import Resolution     | High       | 🟡 Medium  | 85%        | oxc_resolver exists, but edge cases matter |
| CSS Parsing           | High       | 🟡 Medium  | 80%        | lightningcss or custom parser needed       |
| PostCSS Pipeline      | Very High  | 🔴 High    | 70%        | Many plugins to reimplement                |
| Shorthand Expansion   | High       | 🟡 Medium  | 85%        | Finite set of rules, tedious but doable    |
| Autoprefixer          | Very High  | 🔴 High    | 60%        | Complex browserslist + caniuse integration |
| Expression Evaluation | High       | 🟡 Medium  | 80%        | Partial evaluation is well-understood      |
| Code Generation       | Medium     | 🟡 Medium  | 85%        | SWC codegen differs from Babel             |
| Selector Parsing      | Medium     | 🟢 Low     | 90%        | Well-defined CSS spec                      |

---

## Component-by-Component Analysis

### 1. MurmurHash2 ✅ HIGH CONFIDENCE

**Current Implementation:** JavaScript MurmurHash2 GC variant → base-36 string

**Rust Equivalent:** Trivial to port exactly

```rust
pub fn hash(s: &str, seed: u32) -> String {
    // Direct port of the JS implementation
    // Must handle charCodeAt as UTF-16 code units (not UTF-8 bytes!)
    // Output: (result >>> 0).toString(36)
}
```

**Critical Detail:** JavaScript `charCodeAt()` returns UTF-16 code units. Rust strings are UTF-8. You must:

1. Convert input to UTF-16 (`encode_utf16()`)
2. Process as 16-bit code units masked to 8 bits (`& 0xff`)

**Risk:** 🟢 Very Low - deterministic algorithm, easy to test exhaustively.

---

### 2. SWC AST Traversal ✅ HIGH CONFIDENCE

**Current:** Babel visitor pattern with `Program`, `ImportDeclaration`, `JSXElement`, etc.

**SWC Equivalent:** Native `VisitMut` trait

```rust
impl VisitMut for CompiledTransform {
    fn visit_mut_module(&mut self, module: &mut Module) { ... }
    fn visit_mut_import_decl(&mut self, import: &mut ImportDecl) { ... }
    fn visit_mut_jsx_element(&mut self, elem: &mut JSXElement) { ... }
}
```

**Risk:** 🟢 Low - SWC's visitor is well-documented and battle-tested.

---

### 3. Import Resolution ⚠️ MEDIUM RISK

**Current:** Uses `resolve` npm package + custom file reading/parsing

**Proposed:** `oxc_resolver` crate

**Challenges:**

- Must resolve identically to Node's resolution algorithm
- Must handle `exports` field in package.json
- Must support TypeScript path mappings
- Must follow the exact same extension priority (`.ts`, `.tsx`, `.js`, `.jsx`)

**Mitigation Strategy:**

1. Use `oxc_resolver` as foundation
2. Create extensive test suite comparing resolutions
3. Configure to match Node's exact behavior

```rust
use oxc_resolver::{ResolveOptions, Resolver};

let resolver = Resolver::new(ResolveOptions {
    extensions: vec![".ts", ".tsx", ".js", ".jsx"],
    // ... match Node's algorithm exactly
});
```

**Risk:** 🟡 Medium - Edge cases in monorepos, symlinks, and export maps.

---

### 4. CSS Parsing & PostCSS Pipeline 🔴 HIGHEST RISK

This is the **most complex part** of the migration. The current pipeline has 13+ transformation steps.

#### Option A: Reimplement Everything in Rust (Recommended for Identical Output)

**Required Rust Implementations:**

| PostCSS Plugin                 | Rust Equivalent Strategy          |
| ------------------------------ | --------------------------------- |
| `postcss-nested`               | Custom impl (~500 LOC)            |
| `postcss-discard-duplicates`   | Custom impl (~100 LOC)            |
| `postcss-selector-parser`      | Use `lightningcss` or `cssparser` |
| `postcss-values-parser`        | Use `cssparser` crate             |
| `postcss-normalize-whitespace` | Custom impl (~50 LOC)             |
| `postcss-minify-selectors`     | Custom impl (~200 LOC)            |
| `postcss-minify-params`        | Custom impl (~100 LOC)            |
| `postcss-ordered-values`       | Custom impl (~300 LOC)            |
| `postcss-reduce-initial`       | Custom impl + data (~400 LOC)     |
| `postcss-convert-values`       | Custom impl (~300 LOC)            |
| `postcss-colormin`             | Custom impl (~500 LOC)            |
| `postcss-calc`                 | Custom impl (~400 LOC)            |
| `autoprefixer`                 | **See dedicated section**         |

**Custom Compiled Plugins to Port:**

- `atomicify-rules.ts` → `atomicify_rules.rs` (~400 LOC)
- `expand-shorthands/` → `expand_shorthands/` (~1500 LOC total)
- `sort-atomic-style-sheet.ts` → `sort_atomic_style_sheet.rs` (~300 LOC)
- `sort-at-rules.ts` → `sort_at_rules.rs` (~400 LOC)
- `parent-orphaned-pseudos.ts` → `parent_orphaned_pseudos.rs` (~100 LOC)
- `flatten-multiple-selectors.ts` → `flatten_multiple_selectors.rs` (~150 LOC)
- `increase-specificity.ts` → `increase_specificity.rs` (~100 LOC)

#### Option B: FFI to Node's PostCSS (NOT Recommended)

Could call PostCSS via Node FFI, but:

- Defeats performance benefits of native SWC
- Complex IPC/serialization
- Still need Node.js runtime

**Recommendation:** Option A - full Rust rewrite.

---

### 5. Autoprefixer 🔴 CRITICAL COMPLEXITY

**Current:** Uses `autoprefixer` npm package which depends on:

- `browserslist` - Browser target queries
- `caniuse-lite` - Browser feature support database

**The Problem:**
Autoprefixer's output depends on:

1. The browserslist query (defaults to `> 0.5%, last 2 versions, Firefox ESR, not dead`)
2. The caniuse-lite database version at build time
3. The exact date (browser versions change over time)

**Options:**

#### Option 5A: Freeze Autoprefixer Behavior (Recommended)

1. Run autoprefixer in Node.js at a **fixed point in time**
2. Generate a static prefix map: `{ "user-select": ["-webkit-", "-moz-"] }`
3. Embed this map in Rust code
4. Apply prefixes based on the frozen map

**Pros:** Deterministic, simple
**Cons:** Won't adapt to new browsers (acceptable trade-off)

```rust
lazy_static! {
    static ref PREFIX_MAP: HashMap<&'static str, Vec<&'static str>> = {
        // Generated from autoprefixer with frozen browserslist
        hashmap! {
            "user-select" => vec!["-webkit-", "-moz-"],
            "appearance" => vec!["-webkit-", "-moz-"],
            // ... ~100 properties
        }
    };
}
```

#### Option 5B: Port browserslist + caniuse (Not Recommended)

Would require:

- Porting browserslist query parser to Rust
- Embedding caniuse-lite database
- Matching exact version semantics

**Estimated effort:** 3000+ LOC, high maintenance burden.

---

### 6. Shorthand Expansion ⚠️ MEDIUM COMPLEXITY

**Current:** 10 shorthand expanders in `expand-shorthands/`

**Rust Approach:** Direct port with CSS value parsing

```rust
// Example: margin expander
fn expand_margin(value: &str) -> Vec<Declaration> {
    let parts: Vec<&str> = parse_css_values(value);
    match parts.len() {
        1 => vec![
            decl("margin-top", parts[0]),
            decl("margin-right", parts[0]),
            decl("margin-bottom", parts[0]),
            decl("margin-left", parts[0]),
        ],
        2 => vec![
            decl("margin-top", parts[0]),
            decl("margin-right", parts[1]),
            decl("margin-bottom", parts[0]),
            decl("margin-left", parts[1]),
        ],
        // ... 3 and 4 value cases
    }
}
```

**Properties to Expand:**

- margin, padding (TRBL logic)
- overflow (x/y)
- flex (grow/shrink/basis detection)
- flex-flow (direction/wrap)
- outline (color/style/width extraction)
- place-content, place-items, place-self
- text-decoration (color/line/style)
- background (single color only)

**Risk:** 🟡 Medium - Well-defined but tedious. Edge cases in CSS value parsing.

---

### 7. Expression Evaluation ⚠️ MEDIUM COMPLEXITY

**Current:** Evaluates expressions to extract static CSS values

**Required Capabilities:**

- Literal evaluation (strings, numbers, booleans)
- Binary operations (`+`, template literals)
- Object property access
- Array access
- Conditional expressions (for conditional CSS)
- Following imports to resolve values

**Rust Approach:**

```rust
enum EvaluatedValue {
    String(String),
    Number(f64),
    Bool(bool),
    Object(HashMap<String, EvaluatedValue>),
    Array(Vec<EvaluatedValue>),
    Unevaluated(Box<Expr>),  // Keep original for runtime
}

fn evaluate_expr(expr: &Expr, scope: &Scope) -> EvaluatedValue {
    match expr {
        Expr::Lit(lit) => evaluate_literal(lit),
        Expr::Ident(id) => scope.resolve(id),
        Expr::Member(member) => evaluate_member_access(member, scope),
        Expr::Bin(bin) => evaluate_binary(bin, scope),
        Expr::Cond(cond) => EvaluatedValue::Unevaluated(Box::new(expr.clone())),
        // ...
    }
}
```

**Risk:** 🟡 Medium - Must handle same edge cases as Babel's evaluator.

---

### 8. Code Generation ⚠️ MEDIUM RISK

**Current:** Babel's `@babel/generator` with specific comment handling

**Challenge:** SWC's codegen may produce different whitespace/formatting.

**Mitigation:**

1. The output that matters is the **runtime CSS**, not the JS formatting
2. For JS output, configure SWC's codegen to match as closely as possible
3. Create `src/compat/babel_generate.rs` for any Babel-specific output patterns

**Key Differences to Handle:**

- Comment placement (leading vs trailing)
- Parenthesization of expressions
- String quote style
- Trailing commas

```rust
// src/compat/babel_generate.rs
pub struct BabelCompatGenerator {
    // Configuration to match Babel output
}

impl BabelCompatGenerator {
    pub fn generate(&self, module: &Module) -> String {
        // Custom generation logic for Babel-identical output
    }
}
```

**Risk:** 🟡 Medium - May need custom codegen for exact matching.

---

## Proposed Architecture

```
packages/
├── babel-plugin/           # Existing JS implementation (reference)
└── swc-plugin/             # New Rust implementation
    ├── Cargo.toml
    └── src/
        ├── lib.rs                      # Plugin entry point
        ├── babel-plugin.rs             # Main transform (mirrors babel-plugin.ts)
        ├── compat/
        │   ├── mod.rs
        │   ├── babel.rs                # Babel API compatibility layer
        │   └── babel_generate.rs       # Code generation matching Babel
        ├── utils/
        │   ├── mod.rs
        │   ├── hash.rs                 # MurmurHash2 (mirrors hash.ts)
        │   ├── resolve_binding.rs      # Import resolution
        │   ├── evaluate_expression.rs  # Expression evaluation
        │   ├── transform_css_items.rs
        │   ├── build_compiled_component.rs
        │   ├── build_styled_component.rs
        │   ├── hoist_sheet.rs
        │   └── cache.rs                # LRU cache
        ├── css_prop/
        │   ├── mod.rs
        │   ├── index.rs                # CSS prop handler
        │   └── object_literal.rs
        ├── styled/
        │   ├── mod.rs
        │   ├── index.rs
        │   ├── tagged_template.rs
        │   └── call_expression.rs
        ├── class_names/
        │   └── index.rs
        ├── css_map/
        │   └── index.rs
        ├── keyframes/
        │   └── index.rs
        └── css/                        # CSS processing (mirrors packages/css)
            ├── mod.rs
            ├── transform.rs            # Main CSS pipeline
            ├── parser.rs               # CSS parsing
            └── plugins/
                ├── mod.rs
                ├── atomicify_rules.rs
                ├── discard_duplicates.rs
                ├── discard_empty_rules.rs
                ├── parent_orphaned_pseudos.rs
                ├── nested.rs           # postcss-nested equivalent
                ├── normalize_css.rs
                ├── flatten_multiple_selectors.rs
                ├── increase_specificity.rs
                ├── sort_atomic_style_sheet.rs
                ├── autoprefixer.rs     # Frozen prefix map approach
                ├── whitespace.rs
                ├── extract_style_sheets.rs
                └── expand_shorthands/
                    ├── mod.rs
                    ├── margin.rs
                    ├── padding.rs
                    ├── flex.rs
                    ├── overflow.rs
                    ├── outline.rs
                    ├── place_content.rs
                    ├── place_items.rs
                    ├── place_self.rs
                    ├── text_decoration.rs
                    └── background.rs
```

---

## Testing Strategy for Byte-Identical Output

### 1. Snapshot Comparison Framework

```rust
#[test]
fn test_identical_output() {
    let input = r#"
        import { styled } from '@compiled/react';
        const Button = styled.button`color: red;`;
    "#;

    let babel_output = run_babel_plugin(input);  // Via Node FFI or pre-generated
    let swc_output = run_swc_plugin(input);

    assert_eq!(babel_output.code, swc_output.code);
    assert_eq!(babel_output.css, swc_output.css);
}
```

### 2. Fuzzing Strategy

```rust
// Generate random but valid CSS-in-JS inputs
// Compare outputs between Babel and SWC implementations
use arbitrary::Arbitrary;

#[derive(Arbitrary)]
struct FuzzInput {
    css_properties: Vec<(String, String)>,
    has_media_query: bool,
    has_pseudo_selector: bool,
    // ...
}
```

### 3. Production Corpus Testing

1. Collect real-world usage from Atlassian codebase
2. Run both plugins on same inputs
3. Diff all outputs
4. Fix any divergences

---

## Critical Success Factors

### Must Have for 100% Identical Output:

1. **Hash Function Parity**

   - UTF-16 code unit handling in Rust
   - Base-36 encoding matching JS `toString(36)`
   - Test with edge cases (unicode, empty strings, long strings)

2. **CSS Processing Order**

   - Exact same plugin execution order
   - Identical selector normalization
   - Same whitespace handling

3. **Sorting Algorithms**

   - Pseudo-selector order (LVFHA)
   - At-rule sorting (mobile-first breakpoints)
   - Shorthand bucket ordering

4. **Import Resolution**

   - Same file extension priority
   - Same package.json exports handling
   - Same symlink resolution

5. **Autoprefixer Determinism**
   - Frozen prefix map or exact browserslist matching

---

## Potential Blockers

### 1. cssnano Version Sensitivity

cssnano plugins may have version-specific behavior. Solution: Pin exact versions and document.

### 2. Floating Point in CSS

CSS calc() and numeric values may have precision differences. Solution: Match JS's IEEE 754 behavior.

### 3. Unicode Normalization

CSS identifiers with unicode may normalize differently. Solution: Use same normalization form (NFC).

### 4. Source Map Generation

If source maps are required, SWC's source map format may differ. Solution: May need custom source map generation.

---

## Recommended Implementation Order

### Phase 1: Foundation (Week 1-2)

1. ✅ Set up Rust project structure
2. ✅ Port MurmurHash2 with UTF-16 handling
3. ✅ Create test harness comparing Babel vs SWC output
4. ✅ Implement basic AST visitor skeleton

### Phase 2: CSS Pipeline (Week 3-6)

1. Port CSS parsing (use `cssparser` crate)
2. Implement atomicify-rules (core algorithm)
3. Port all shorthand expanders
4. Implement sorting algorithms
5. Port postcss-nested equivalent
6. Implement normalize-css plugins
7. Create frozen autoprefixer

### Phase 3: Babel Plugin Logic (Week 7-10)

1. Import resolution with oxc_resolver
2. Expression evaluation
3. styled() handler
4. css() handler
5. cssMap() handler
6. ClassNames component handler
7. CSS prop handler
8. keyframes() handler

### Phase 4: Code Generation & Polish (Week 11-12)

1. Babel-compatible code generation
2. Metadata structure
3. Edge case handling
4. Performance optimization

### Phase 5: Validation (Week 13-14)

1. Run against production corpus
2. Fix all divergences
3. Fuzzing campaign
4. Documentation

---

## Alternative Approaches Considered

### 1. WASM Plugin Instead of Native

**Rejected:** Performance overhead, doesn't leverage SWC's native speed.

### 2. Hybrid Approach (Rust + Node PostCSS)

**Rejected:** Complex IPC, still requires Node runtime, defeats purpose.

### 3. Use lightningcss for Everything

**Considered:** lightningcss is fast but may not match PostCSS output exactly. Could use as foundation but would need customization.

### 4. Accept Minor Differences

**Rejected:** User requirement is 100% byte-identical. No exceptions.

---

## Conclusion

This migration is **feasible** with:

- **High confidence** in: hashing, AST traversal, sorting, shorthand expansion
- **Medium confidence** in: import resolution, expression evaluation, code generation
- **Lower confidence** in: exact PostCSS plugin parity, autoprefixer determinism

The **frozen autoprefixer approach** is the key insight that makes deterministic output achievable without porting the entire browserslist/caniuse ecosystem.

**Recommendation:** Proceed with Phase 1 to validate the approach with a minimal end-to-end test before committing to full implementation.
