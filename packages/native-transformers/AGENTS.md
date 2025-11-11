## Facts
packages/native-transformers/compiled_babel Is designed to be a SWC drop-in replacement for packages/babel-plugin
packages/native-transformers/compiled_strip_runtime is designed to be SWC drop-in replacement packages/babel-plugin-strip-runtime

"Drop in replacement": Means roughly same file/folder structure, all the same bugs,features and options. Except those introduced by "swc" itself (as opposed to babel).
Hashes and amount of style rules generated should be identical between the two implementations.

Both of these will run as native Rust transformers; Never as WASM plugins; They return metadata as a return from the "transform step"
because SWC doesn't have a metadata API like babel does.

We're in the process of testing

Fixture pattern
- Folder name = The test case name
- in.jsx = The test case code.
- out.js = What we're expecting
- config.json = Any specific plugin config for that test case.
- babel-style-rules.json = metadata.styleRules coming from babel-plugin; (check parcel-transformer for example usage)
- swc-style-rules.json = style rules coming from swc transformer returned through metadata.
- babel-out.js = The output from running both plugins in order (see packages/parcel-trasnformer for example usage)
- actual.js = The output from running BOTH swc transformers in same order as babel.



