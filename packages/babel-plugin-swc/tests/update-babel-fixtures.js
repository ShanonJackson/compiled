#!/usr/bin/env node
const fs = require('fs');
const path = require('path');
const { transformSync } = require('@babel/core');
const compiledBabelPlugin = require('@compiled/babel-plugin');
const stripRuntimeBabelPlugin = require('@compiled/babel-plugin-strip-runtime');

const repoRoot = path.resolve(__dirname, '..', '..', '..');
const fixturesRoot = path.resolve(__dirname, 'fixtures');

const fixtureArgs = process.argv.slice(2);

const fixtureNames =
  fixtureArgs.length > 0
    ? fixtureArgs
    : fs
        .readdirSync(fixturesRoot)
        .filter((entry) => fs.statSync(path.join(fixturesRoot, entry)).isDirectory());

if (fixtureNames.length === 0) {
  console.error('No fixtures found.');
  process.exit(1);
}

const ensureNewline = (value) => (value.endsWith('\n') ? value : `${value}\n`);

const transformFixture = (code, filename, { run, runtime = 'automatic' }) => {
  const bake = run === 'both' || run === 'bake';
  const extract = run === 'both' || run === 'extract';

  return transformSync(code, {
    babelrc: false,
    configFile: false,
    cwd: repoRoot,
    filename,
    parserOpts: {
      plugins: ['jsx', 'typescript'],
    },
    plugins: [
      ...(bake
        ? [[compiledBabelPlugin, { importReact: runtime === 'classic', optimizeCss: false }]]
        : []),
      ...(extract
        ? [[stripRuntimeBabelPlugin, { compiledRequireExclude: true, sortAtRules: true, sortShorthand: true }]]
        : []),
    ],
    presets: [['@babel/preset-react', { runtime }]],
  });
};

for (const name of fixtureNames) {
  const fixtureDir = path.join(fixturesRoot, name);
  const inputPath = path.join(fixtureDir, 'in.jsx');

  if (!fs.existsSync(inputPath)) {
    continue;
  }

  const source = fs.readFileSync(inputPath, 'utf8');

  const bakeResult = transformFixture(source, inputPath, { run: 'bake' });
  if (!bakeResult || typeof bakeResult.code !== 'string') {
    throw new Error(`Failed to generate Babel output for fixture: ${name}`);
  }

  fs.writeFileSync(path.join(fixtureDir, 'babel-out.js'), ensureNewline(bakeResult.code));

  const extractResult = transformFixture(source, inputPath, { run: 'both' });
  const metadata = (extractResult && extractResult.metadata) || {};
  const styleRules = Array.isArray(metadata.styleRules) ? metadata.styleRules : [];

  fs.writeFileSync(
    path.join(fixtureDir, 'babel-style-rules.json'),
    ensureNewline(JSON.stringify(styleRules, null, 2))
  );
}
