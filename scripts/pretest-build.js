#!/usr/bin/env node
const { spawnSync } = require('child_process');
const { existsSync } = require('fs');
const { join } = require('path');

if (process.env.SKIP_PRETEST_BUILD) {
  process.exit(0);
}

const repoRoot = join(__dirname, '..');
const requiredArtifacts = [
  join(repoRoot, 'packages', 'react', 'dist', 'cjs', 'runtime', 'style.js'),
  join(repoRoot, 'packages', 'babel-plugin', 'dist', 'index.js'),
  join(repoRoot, 'packages', 'babel-plugin-strip-runtime', 'dist', 'index.js'),
];

const allArtifactsPresent = requiredArtifacts.every((artifact) => existsSync(artifact));

if (allArtifactsPresent) {
  process.exit(0);
}

const result = spawnSync('yarn', ['build'], {
  stdio: 'inherit',
  shell: process.platform === 'win32',
});

if (result.status !== 0) {
  process.exit(result.status ?? 1);
}
