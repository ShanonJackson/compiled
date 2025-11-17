Learnings while matching Babel 1:1

- Css Prop Traversal
  - Symptom: Css prop wasn’t transforming inside nested JSX (e.g. <div><span css=… /></div>).
  - Root cause: Our SWC visitor only handled Expr::JSXElement in visit_mut_expr and missed JSX children, which aren’t expressions. Babel runs the transform on JSXOpeningElement, which visits every element.
  - Fix (implemented): Add a dedicated JSXElement visitor that walks children depth‑first and applies the css‑prop transform on each element in place. This mirrors Babel’s per‑element behavior and fixed the double‑export style rules.

- Strip Runtime Traversal (CC/CS wrappers)
  - Symptom: In double-export, some CC/CS wrappers were still present in out.jsx whereas Babel’s output had them stripped consistently.
  - Investigation: Our compiled_strip_runtime transform replaces CC only when the JSX element occurs as an Expr (visit_mut_expr). Nested CC elements inside a JSX tree are not visited because JSX children aren’t expressions. Babel’s plugin visits JSXElement nodes directly and replaces CC at any depth.
  - Deviation location: packages/native-transformers/compiled_strip_runtime/src/strip_runtime.rs — VisitMut implementation only covers Expr::JSXElement and misses nested children. Replacement helpers exist (replace_cc_jsx, replace_cc_jsxs_call), but they need to be invoked from a JSXElement walker as well.
  - Proposed fix (for parity): Add a visit_mut_jsx_element that traverses children and replaces nested CC elements in place (constructing the correct JSXElementChild). Keep behavior identical to Babel’s logic for JSXElement and CallExpression cases.

- Logging for Targeted Fixture Debugging
  - Added COMPILED_CLI_TRACE gated logging to the css‑prop path (element names, attributes, found css) and the resolution path (identifier resolution, export alias mapping) to align behavior precisely with Babel without guessing.

These changes and findings help keep the SWC port behaviorally identical to the Babel implementation at the correct abstraction layers (per-element JSX traversal and resolveBinding semantics), reducing the risk of surface-level fixes.

- CSS Selector Normalization
  - Symptom: Fixtures such as `css-map-child-selector`, `css-content-selectors`, and `css-child-combinator-spacing` produced different hashes/selector text (missing spaces around combinators or pseudo selectors).
  - Root cause: Our Rust `normalize_selector` inserted a space before *every* combinator after encountering `&`, whereas Babel only inserts spaces for descendant combinators and preserves the absence of whitespace before `>`, `+`, `~`, `||`, or pseudo selectors when they were written without it.
  - Fix: Re-implemented `normalize_selector`/`normalize_ampersand_combinators` exactly like Babel – collapse only the leading whitespace after explicit combinators, keep descendant spaces, and insert the `&` in pseudo-leading selectors. Applied the same helper in both the SWC pipeline and the PostCSS-engine path so they cannot drift.

- Parent-Orphaned Pseudos Interaction
  - Symptom: Selectors like `span &:before` lost the nesting selector once normalization ran, causing Babel and SWC hashes to diverge.
  - Root cause: The parent-orphaned pseudos plugin only inserted `&` for pseudo-only selectors, but Babel’s pipeline also considers selectors where a descendant combinator precedes the nesting selector.
  - Fix: Extend `parent_orphaned_pseudos` to insert a nesting selector when the first compound selector lacks one but a descendant combinator exists later, mirroring Babel’s behavior and allowing normalization to proceed identically.
