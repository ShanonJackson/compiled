# PLAN

## [Completed] Baseline workspace setup
- Align Yarn configuration with the monorepo requirements and ensure build/test scripts execute reliably.
- Establish the native Rust crate scaffolding for the SWC transformer.

## [Completed] Core transform infrastructure
- Mirror the Babel plugin module layout in Rust and port the transform state, import tracking, and evaluation helpers.
- Implement css/styled/keyframes/cssMap/classNames lowering plus css/xcss prop support with extraction aware style-rule collection.

## [Completed] PostCSS normalisation parity
- Port the `@compiled/css` normalisation pipeline (cssnano preset + custom helpers) so property/value hashing matches Babel in all cases.
- Replicate shorthand expansion, selector/at-rule minification, and production-only optimisations behind the `optimizeCss` flag.

## [Completed] Resolver integration parity
- Surface the `oxc_resolver` powered import resolution in the transform state so evaluated imports match the Babel cache semantics.
- Honour user-supplied resolver options and extend fixture coverage for resolution edge cases.

## Hash parity verification
- Expand fixture coverage and automated comparisons against Babel outputs (code + style rules) to ensure hashes remain identical.
- Integrate regression checks that compare emitted sheets/class names for complex runtime scenarios.
- Capture keyframes tagged-template lowering so extracted fixtures include the generated `@keyframes` sheet alongside the runtime class rule.

## Packaging & documentation
- Document the regeneration workflows, extraction outputs, and integration steps for consumers adopting the native plugin.
- Align crate metadata and publishing scripts with the monorepo release process.
