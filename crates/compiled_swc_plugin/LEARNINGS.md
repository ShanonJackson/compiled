# CSS Map Hashing Learnings

- CSS map selectors that start with a combinator (for example `> button`) must preserve the leading space when hashing and when emitting CSS; otherwise the selector hash diverges from Babel (`_aonsh9n0` vs `_a8iph9n0`). The fix keeps `compress_selector_for_hash`/`compress_selector_for_output` aware of a `preserve_leading_combinator_space` option that is enabled for CSS map transformations only.
- Styled/regular `css` selectors should continue to collapse the leading space so we do not regress fixtures like `css-nested-selectors`. Adding the flag instead of globally changing the behaviour prevents those regressions.
- When adding new behaviour that affects selector hashing, regenerate the affected fixtures and re-run `node jira/analyze-style-rules.js` so the large-style-rule diff stays clean.
- The whitespace canonicalisation in `jira/analyze-style-rules.js` now strips spaces before `> + ~` to match the SWC output; keep that in mind when comparing Babel/SWC JSON.
