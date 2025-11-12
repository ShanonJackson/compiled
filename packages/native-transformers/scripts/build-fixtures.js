#!/usr/bin/env node
const { spawnSync } = require('child_process');
const path = require('path');

const repoRoot = path.resolve(__dirname, '..');
const cargoArgs = ['build', '-p', 'fixtures_cli', '--release'];

// Force-disable LTO for this build to avoid proc-macro + LTO conflicts when users set global flags.
const env = { ...process.env, RUSTFLAGS: '-Clto=off' };

const res = spawnSync('cargo', cargoArgs, {
  cwd: repoRoot,
  stdio: 'inherit',
  env,
  shell: process.platform === 'win32',
});

if (res.status !== 0) {
  process.exitCode = res.status || 1;
}

