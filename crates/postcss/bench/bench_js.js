#!/usr/bin/env node
// JS PostCSS 8.4.31 benchmark — parse → stringify roundtrip
// Outputs JSON: { file, size_kb, iterations, total_ms, avg_us, ops_per_sec, output_hash }

const fs = require('fs');
const path = require('path');
const postcss = require('postcss');
const crypto = require('crypto');

const file = process.argv[2];
if (!file) {
  console.error('Usage: node bench_js.js <css-file> [iterations]');
  process.exit(1);
}

const iterations = parseInt(process.argv[3] || '100', 10);
const css = fs.readFileSync(file, 'utf-8');
const sizeKb = (Buffer.byteLength(css) / 1024).toFixed(1);

// Force actual parse + stringify (postcss([]).process with zero plugins
// uses NoWorkResult which skips parsing entirely — unfair comparison).
// We use postcss.parse() + root.toString() to measure real work.

// Warmup (5 iterations to trigger JIT).
for (let i = 0; i < 5; i++) {
  postcss.parse(css, { from: undefined }).toString();
}

// Benchmark.
let output;
const start = process.hrtime.bigint();
for (let i = 0; i < iterations; i++) {
  output = postcss.parse(css, { from: undefined }).toString();
}
const elapsed = process.hrtime.bigint() - start;

const totalMs = Number(elapsed) / 1e6;
const avgUs = (Number(elapsed) / iterations) / 1e3;
const opsPerSec = (iterations / (totalMs / 1000)).toFixed(0);
const hash = crypto.createHash('md5').update(output).digest('hex').slice(0, 12);

console.log(JSON.stringify({
  file: path.basename(file),
  size_kb: parseFloat(sizeKb),
  iterations,
  total_ms: Math.round(totalMs),
  avg_us: Math.round(avgUs),
  ops_per_sec: parseInt(opsPerSec),
  output_hash: hash,
}));

// Also write the output to a file for correctness comparison.
const outFile = file.replace('.css', '.js_output.css');
fs.writeFileSync(outFile, output);
