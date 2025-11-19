#!/usr/bin/env node

const fs = require('fs');
const fsp = require('fs/promises');
const path = require('path');
const { spawnSync } = require('child_process');
const { performance } = require('perf_hooks');

const babel = require('@babel/core');

const repoRoot = path.resolve(__dirname, '..', '..', '..');
process.chdir(repoRoot);

const fixtureRoot = path.join(
  repoRoot,
  'packages',
  'native-transformers',
  'tests',
  'fixtures'
);

const BABEL_OPTIONS = {
  babelrc: false,
  configFile: false,
  sourceMaps: false,
  ast: false,
  caller: { name: 'compiled-native-transformers-bench' },
};

function formatSeconds(ms) {
  return (ms / 1000).toFixed(2);
}

async function readFixtureConfig(dir) {
  try {
    const text = await fsp.readFile(path.join(dir, 'config.json'), 'utf8');
    return JSON.parse(text);
  } catch (_e) {
    return {};
  }
}

async function loadFixtureData(names) {
  const fixtures = [];
  for (const name of names) {
    const dir = path.join(fixtureRoot, name);
    const inputPath = path.join(dir, 'in.jsx');
    const inputCode = await fsp.readFile(inputPath, 'utf8');
    const cfg = await readFixtureConfig(dir);
    fixtures.push({
      name,
      dir,
      inputPath,
      inputCode,
      compiledOptions: {
        cache: false,
        optimizeCss: true,
        importReact: true,
        extract: typeof cfg.extract === 'boolean' ? cfg.extract : true,
        ...(cfg.classNameCompressionMap
          ? { classNameCompressionMap: cfg.classNameCompressionMap }
          : {}),
      },
    });
  }
  return fixtures;
}

async function selectFixtures(args) {
  const entries = await fsp.readdir(fixtureRoot, { withFileTypes: true });
  const all = entries
    .filter((entry) => entry.isDirectory() && !entry.name.startsWith('.'))
    .map((entry) => entry.name);
  if (!args || args.length === 0) {
    return all;
  }
  const requested = args
    .flatMap((arg) => arg.split(','))
    .map((name) => name.trim())
    .filter(Boolean);
  const unknown = requested.filter((name) => !all.includes(name));
  if (unknown.length > 0) {
    throw new Error(
      `Unknown fixture name(s): ${unknown.join(', ')}\nAvailable: ${all.join(', ')}`
    );
  }
  return requested;
}

async function benchmarkBabel(fixtures) {
  console.log(`[bench] running Babel on ${fixtures.length} fixtures`);
  const perFixture = [];
  const start = performance.now();
  for (const fixture of fixtures) {
    const label = fixture.name;
    const t0 = performance.now();
    const result = babel.transformSync(fixture.inputCode, {
      ...BABEL_OPTIONS,
      filename: fixture.inputPath,
      plugins: [
        [require.resolve('@compiled/babel-plugin'), fixture.compiledOptions],
        [require.resolve('@compiled/babel-plugin-strip-runtime'), { compiledRequireExclude: true }],
      ],
    });
    if (!result || typeof result.code !== 'string') {
      throw new Error(`Babel failed for fixture ${label}`);
    }
    const dur = performance.now() - t0;
    perFixture.push(dur);
    console.log(`[bench] ${label}: babel ${formatSeconds(dur)}s`);
  }
  return { total: performance.now() - start, perFixture };
}

function ensureSwcBinary() {
  const bin = path.join(
    repoRoot,
    'packages',
    'native-transformers',
    'target',
    'release',
    process.platform === 'win32' ? 'fixtures_cli.exe' : 'fixtures_cli'
  );
  if (fs.existsSync(bin)) {
    return bin;
  }
  console.log('[bench] building fixtures_cli');
  const build = spawnSync('cargo', ['build', '-p', 'fixtures_cli', '--release'], {
    cwd: path.join(repoRoot, 'packages', 'native-transformers'),
    stdio: 'inherit',
    shell: process.platform === 'win32',
  });
  if (build.status !== 0) {
    throw new Error('Failed to build fixtures_cli binary for SWC benchmark.');
  }
  return bin;
}

function createSwcEnv() {
  const env = { ...process.env };
  try {
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const babelPkg = require('@compiled/babel-plugin/package.json');
    if (babelPkg && typeof babelPkg.version === 'string' && babelPkg.version) {
      env.TEST_PKG_VERSION = babelPkg.version;
    }
  } catch (_e) {
    // ignore missing pkg
  }
  return env;
}

async function benchmarkSwc(fixtures, bin) {
  const env = createSwcEnv();
  console.log(`[bench] running SWC on ${fixtures.length} fixtures`);
  const perFixture = [];
  const start = performance.now();
  for (const fixture of fixtures) {
    const label = fixture.name;
    const t0 = performance.now();
    const run = spawnSync(bin, [fixture.inputPath], {
      cwd: path.join(repoRoot, 'packages', 'native-transformers'),
      encoding: 'utf8',
      shell: process.platform === 'win32',
      env,
    });
    if (run && typeof run.stderr === 'string' && run.stderr.length) {
      try {
        process.stderr.write(run.stderr);
      } catch (_e) {
        // ignore write errors
      }
    }
    if (run.status !== 0) {
      throw new Error(`SWC failed for fixture ${label}`);
    }
    const dur = performance.now() - t0;
    perFixture.push(dur);
    console.log(`[bench] ${label}: swc ${formatSeconds(dur)}s`);
  }
  return { total: performance.now() - start, perFixture };
}

async function main() {
  const fixturesToRun = await selectFixtures(process.argv.slice(2).filter(Boolean));
  if (fixturesToRun.length === 0) {
    console.log('No fixtures to benchmark.');
    return;
  }
  const fixtureData = await loadFixtureData(fixturesToRun);
  const babelStats = await benchmarkBabel(fixtureData);
  const swcBinary = ensureSwcBinary();
  const swcStats = await benchmarkSwc(fixtureData, swcBinary);

  console.log('');
  console.log('Benchmark summary:');
  console.log(
    ` - Babel total ${formatSeconds(babelStats.total)}s (${formatSeconds(
      babelStats.total / fixtureData.length
    )}s avg)`
  );
  console.log(
    ` - SWC   total ${formatSeconds(swcStats.total)}s (${formatSeconds(
      swcStats.total / fixtureData.length
    )}s avg)`
  );
  const delta = swcStats.total - babelStats.total;
  const faster = delta < 0 ? 'SWC faster' : delta > 0 ? 'Babel faster' : 'Tie';
  console.log(` - ${faster} by ${formatSeconds(Math.abs(delta))}s`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
