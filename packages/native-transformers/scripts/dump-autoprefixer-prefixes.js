#!/usr/bin/env node
/*
  Generates a JSON snapshot of Autoprefixer’s computed prefixes map
  (equivalent to `require('autoprefixer/data/prefixes')`).

  Output: packages/native-transformers/compiled_babel/autoprefixer_data/prefixes.json

  This file is used by the Rust port to ensure 1:1 data parity with the
  vendored Autoprefixer version in packages/postcss-plugin-sources/autoprefixer.
*/
const fs = require('fs');
const path = require('path');

const repoRoot = path.resolve(__dirname, '..', '..', '..');
const src = path.join(
  repoRoot,
  'packages',
  'postcss-plugin-sources',
  'autoprefixer',
  'data',
  'prefixes.js'
);

const outDir = path.join(
  repoRoot,
  'packages',
  'native-transformers',
  'compiled_babel',
  'autoprefixer_data'
);
const outFile = path.join(outDir, 'prefixes.json');

function main() {
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  const prefixes = require(src);
  if (!prefixes || typeof prefixes !== 'object') {
    throw new Error('autoprefixer data/prefixes.js did not export an object');
  }
  fs.mkdirSync(outDir, { recursive: true });
  fs.writeFileSync(outFile, JSON.stringify(prefixes, null, 2) + '\n', 'utf8');
  console.log('[dump-autoprefixer-prefixes] wrote', path.relative(repoRoot, outFile));
}

main();

