#!/usr/bin/env node
/*
  Dumps caniuse-lite agents JSON Autoprefixer uses to compute prefixes
  (equivalent to require('caniuse-lite/dist/unpacker/agents').agents).

  Output: packages/native-transformers/compiled_babel/autoprefixer_data/agents.json
*/
const fs = require('fs');
const path = require('path');

const repoRoot = path.resolve(__dirname, '..', '..', '..');
const outDir = path.join(
  repoRoot,
  'packages',
  'native-transformers',
  'compiled_babel',
  'autoprefixer_data'
);
const outFile = path.join(outDir, 'agents.json');

function main() {
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  const { agents } = require('caniuse-lite/dist/unpacker/agents');
  if (!agents || typeof agents !== 'object') {
    throw new Error('caniuse-lite agents missing');
  }
  fs.mkdirSync(outDir, { recursive: true });
  fs.writeFileSync(outFile, JSON.stringify(agents, null, 2) + '\n', 'utf8');
  console.log('[dump-autoprefixer-agents] wrote', path.relative(repoRoot, outFile));
}

main();

