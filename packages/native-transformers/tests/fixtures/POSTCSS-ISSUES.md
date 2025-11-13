- Using the vendored sources under packages/postcss-plugin-sources/postcss-ordered-values and mirroring their file/folder structure in normalize_css_engine/ordered_values/rules/*.
- Building values via value_parser nodes with explicit Space nodes exactly where JS emits them (via cssnano-utils getArguments + getValue equivalents).
- Ensuring border and boxShadow reducers rebuild “1px solid #000” and “0 0 0 1px rgba(0,0,0,.1)” byte-identically.
- Keeping colormin’s candidate selection and function→word spacing identical to colord/minify so adjacent tokens never merge.


