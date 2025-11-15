Instructions.
We're replacing two existing babel-plugins with native Rust SWC transformers, packages/babel-plugin and packages/babel-plugin-strip-runtime.
packages/native-transformers/compiled_babel Is designed to be a SWC drop-in replacement for packages/babel-plugin [verified same (hopefully)]
packages/native-transformers/compiled_strip_runtime is designed to be SWC drop-in replacement packages/babel-plugin-strip-runtime [verified same (hopefully)]
Correctness is the goal here, which is defined as a 1:1 faithful identical port from js to rust including all behaviours, bugs and hash equality.
To achieve this we've replicated 'hopefully' the entire file/folder structure of the originals in Rust, AND re-implemented 'postcss' AND all it's plugins entirely in Rust for the versions of the libraries that the originals were using.
It's 'possible' that small differences may be found in our ports of these (postcss port, babel-plugin port or babel-plugin-strip-runtime port) however whenever we find these differences we need to resolve them in a way that makes the output identical to the original babel-plugins.
However, the WAY we resolve them is important, when we find a difference we first need to check the 'input' to see the exact point in the packages/babel-plugin that input becomes wrong;
That's where we need to fix the issue, because our goal is to have a 1:1 replica including all bugs, if we just start fixing issues bespokely we will diverge and we've failed.
Whenever we can't achieve a 1:1 replica (i.e because Babel has an API that SWC doesn't have) then you need to raise that and suggest a way forwards. I.E We had clear comment: // COMPAT: Babel does x we need to replicate that here with Y

We've even replicated the ENTIRE postcss pipeline of the original including all plugins 1:1 in Rust, so that the input to our hashing functions is identical to the original babel-plugin.

Again to be very clear, All code and libraries in the original are translated 1:1 to Rust, including all bugs and quirks; We are NEVER 'patching'
behaviours in bespoke places, if you ever need to deviate behaviour you MUST raise that as an issue and we will discuss how to handle it.

When this work is finished every input file will produce identical output files through babel or through swc. In all cases. Every single time.
If you ever need the original JS source code of anything to compare, just ask me and i'll provide it. If everything is correctly ported 1:1 then the Sum of everything is a 1:1 replica.

If you ever have to deviate from the original file/folder OR code logic because it can't be 1:1 you need to raise it immediately with a work around for approval.

Always ask yourself, why does this difference exist? It exists because we failed to port something 1:1; Which means when we fix it we need to fix it how it works in the original.



requirements.md
- swc_core has to be on 26.3.4 [This requirement is currently met]
- use oxc_resolver which is a replacement for webpack-custom-resolve [currently installed]
- yarn 1.22.22 (is the yarn version used in the monorepo); node version is 20.15.1 (node version used in the monorepo)
- This css-in-js plugin supports using imported values in css-in-js syntax; When you need to implement this behaviour use oxc_resolver 11.13.1 AND SWC to walk the dependency tree to find the exported value.
- replicates logic verbatim AND file/folder structure; This is to ensure any bugs in old carry across to new. For things like babel.metadata that don't have a swc equivilent we should return metadata as a struct from the native rust transformer function.
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
- Ensure you fix warnings emitted in terminal when we run fixtures_cli -- release - We want a clean production copy.

Testing Plan.md
- Fixtures are going to be the test strategy for catching regressions between new plugins.
- tests/fixtures/<test case name>, in.jsx = input code, out.jsx = output code after our plugins, babel-out.jsx output code after babel-plugins, babel-style-rules.json = JUST styleRules from babel, swc-style-rules.json = JUST styleRules from SWC
- This is the command to update AND run fixtures for latest code: packages/native-transformers/scripts/update-fixtures.js
- It checks for ANY miss match in out.jsx compared to babel-out.jsx AND ANY missmatch in style-rules json files.


Resources:
packages/postcss-plugin-sources  - You can find all the original post-css plugin source code here for the js implementations:
cargo build --manifest-path packages/native-transformers/Cargo.toml -p fixtures_cli --release         (this builds fixtures)
node packages/native-transformers/scripts/update-fixtures.js                (this runs the fixtures update script)


