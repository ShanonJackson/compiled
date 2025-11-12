Instructions.
Your goal is to implement PLAN.md marking the headings [Completed] when each task is done.
We're replacing two existing babel-plugins with native Rust SWC transformers, packages/babel-plugin and packages/babel-plugin-strip-runtime.
Correctness is the goal here, but also performance. We're replicating two existing babel-plugins IDENTICALLY including all files/folders/behaviours AND bugs. Hashes MUST remain the same in output;
However cosmetic differences like those introduced from using swc instead of babel are acceptable.
These plugins will run as native Rust transformers NOT WASM plugins; And the packages/babel-plugin equivilent can emit "style-rules" as a return object to mimic babels 'metadata' of existing plugin.

Whenever we discover differences beteween the behaviours of the two implementations; It's important we resolve the difference in a way that's faithful to the original, so the end result doesn't
cause more drift in logic/behaviour. Our job is to translate the original in both implementation AND bugs AND AST to the new copy to be a drop-in replacement.

Any differences should be in the same location as the original when fixed and we leave a comment say like
// COMPAT: This is because babel-plugin has X behaviour and we need to replicate it here exactly.


requirements.md
- swc_core has to be on 26.3.4 [This requirement is currently met]
- use oxc_resolver which is a replacement for webpack-custom-resolve [currently installed]
- yarn 1.22.22 (is the yarn version used in the monorepo); node version is 20.15.1 (node version used in the monorepo)
- This css-in-js plugin supports using imported values in css-in-js syntax; When you need to implement this behaviour use oxc_resolver 11.13.1 AND SWC to walk the dependency tree to find the exported value.
- replicates logic verbatim AND file/folder strucutre; This is to ensure any bugs in old carry across to new. For things like babel.metadata that don't have a swc equivilent we should return metadata as a struct from the native rust transformer function.
- hashing must remain identical for both the <selector> and <value> portion of the hash.
- postcss is a library that normalizes input; We need to replicate this library in postcss.rs for the version the babel-plugin is using. In Rust; Again it has to be verbatim and handle input/output identically to existing.
- "style-rules" should be emitted when extract is true; Similar to how packages/parcel-transformer does it.
- Comprehensive testing to ensure it's EXACTLY a drop-in replacement is critical to ensure that as we're working we have a feedback system for regressions;
- style rules need to be atomic and deduplicated (same as existing); When we replicate file/folder strucutre this will be apprant.
- css/cssMap/styled/keyframes will all work identically; resolution order needs to remain the same.
- caching of imports needs to match existing bable-plugin; As this will be performance issue.
- Performance is a priority; But correctness is our #1 priority as if it's not "the same" (except for cosmetic differences between babel/swc output irrespective of our plugin); Then it's unusuable.
- Hashes have to remain the same, this isn't a cosmetic difference. In order for them to be the same postcss.rs AND the input structure into the hash MUST BE THE SAME
- Our fixtures tests will verify out.js (generated from actual.js) matches babel except cosmetically; And will verify style-rules are the same including hashes.
- The 'transform' function will emit the new AST AND will emit 'style-rules' as a returned data type, basically anything that  is on babel's metadata will be returned as a return value of the transform pass.
- Errors should be reported with https://rustdoc.swc.rs/swc_common/errors/struct.Handler.html identically to as they are now.
- Javascript support is NOT required; Again this plugin will run natively via Rust on a SWC AST; Therefore we are never compiling this to WASM.


Testing Plan.md
- Fixtures are going to be the test strategy for catching regressions between new plugins.
- tests/fixtures/<test case name>, in.jsx = input code, out.jsx = output code after our plugins, babel-out.jsx output code after babel-plugins, babel-style-rules.json = JUST styleRules from babel, swc-style-rules.json = JUST styleRules from SWC
- This is the command to update AND run fixtures for latest code: packages/native-transformers/scripts/update-fixtures.js
- It checks for ANY missmatch in out.jsx compared to babel-out.jsx AND ANY missmatch in style-rules json files.



