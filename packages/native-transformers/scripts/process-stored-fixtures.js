#!/usr/bin/env node

const fs = require('fs');
const fsp = require('fs/promises');
const path = require('path');

const repoRoot = path.resolve(__dirname, '..', '..', '..');
process.chdir(repoRoot);

// Processes the temporary stored fixtures and moves the ones that match to fixtures-passing.
const babel = require('@babel/core');

const fixturesRoot = path.join(
  repoRoot,
  'packages',
  'native-transformers',
  'tests',
  'stored',
  'fixtures'
);
const passingRoot = path.join(
  repoRoot,
  'packages',
  'native-transformers',
  'tests',
  'stored',
  'fixtures-passing'
);

function splitTopLevelSegments(body) {
  const segments = [];
  let depth = 0;
  let start = -1;
  for (let i = 0; i < body.length; i++) {
    const ch = body[i];
    if (depth === 0 && start === -1) {
      if (/\S/.test(ch)) {
        start = i;
      } else {
        continue;
      }
    }
    if (ch === '{') {
      depth++;
    } else if (ch === '}') {
      depth--;
      if (depth === 0 && start !== -1) {
        segments.push(body.slice(start, i + 1));
        start = -1;
      }
    }
  }
  return segments;
}

function normalizeStyleRule(rule) {
  const trimmed = (rule || '').trim();
  if (!trimmed.startsWith('@media')) {
    return trimmed;
  }
  const open = trimmed.indexOf('{');
  if (open === -1) {
    return trimmed;
  }
  let depth = 0;
  let close = -1;
  for (let i = open; i < trimmed.length; i++) {
    const ch = trimmed[i];
    if (ch === '{') {
      depth++;
    } else if (ch === '}') {
      depth--;
      if (depth === 0) {
        close = i;
        break;
      }
    }
  }
  if (close === -1) {
    return trimmed;
  }
  const prefix = trimmed.slice(0, open + 1);
  const body = trimmed.slice(open + 1, close);
  const suffix = trimmed.slice(close);
  const segments = splitTopLevelSegments(body)
    .map((segment) => normalizeStyleRule(segment.trim()))
    .filter(Boolean)
    .sort();
  if (segments.length <= 1) {
    return trimmed;
  }
  return `${prefix}${segments.join('')}${suffix}`.trim();
}

const BABEL_OPTIONS = {
  babelrc: false,
  configFile: false,
  sourceMaps: false,
  ast: false,
  caller: { name: 'compiled-native-transformers-fixtures' },
  plugins: [
    [
      require.resolve('@compiled/babel-plugin'),
      { cache: false, optimizeCss: true, importReact: true, extract: true },
    ],
    [require.resolve('@compiled/babel-plugin-strip-runtime'), { compiledRequireExclude: true }],
  ],
};

async function readFixtureConfig(dir) {
  try {
    const text = await fsp.readFile(path.join(dir, 'config.json'), 'utf8');
    return JSON.parse(text);
  } catch (_e) {
    return {};
  }
}

async function writeFileIfChanged(filePath, content) {
  const normalized = typeof content === 'string' ? content : JSON.stringify(content, null, 2);
  let existing;
  try {
    existing = await fsp.readFile(filePath, 'utf8');
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
  }
  if (existing !== normalized) {
    await fsp.mkdir(path.dirname(filePath), { recursive: true });
    await fsp.writeFile(filePath, normalized + (normalized.endsWith('\n') ? '' : '\n'));
  }
}

async function generateBabelOutputs(fixtureDir, inputCode, inputPath) {
  const label = path.basename(fixtureDir);
  const t0 = Date.now();
  console.log(`[stored fixtures] ${label}: babel`);
  const cfg = await readFixtureConfig(fixtureDir);
  const compiledOptions = {
    cache: false,
    optimizeCss: true,
    importReact: true,
    extract: typeof cfg.extract === 'boolean' ? cfg.extract : true,
    ...(cfg.classNameCompressionMap
      ? { classNameCompressionMap: cfg.classNameCompressionMap }
      : {}),
  };

  const result = babel.transformSync(inputCode, {
    ...BABEL_OPTIONS,
    filename: inputPath,
    plugins: [
      [require.resolve('@compiled/babel-plugin'), compiledOptions],
      [
        require.resolve('@compiled/babel-plugin-strip-runtime'),
        { compiledRequireExclude: true },
      ],
    ],
  });
  console.log(
    `[stored fixtures] ${label}: babel done ${((Date.now() - t0) / 1000).toFixed(1)}s`
  );

  if (!result || typeof result.code !== 'string') {
    throw new Error(`Failed to transform fixture at ${fixtureDir}`);
  }

  const metadata = (result.metadata && result.metadata.styleRules) || [];
  return {
    styleRules: Array.isArray(metadata) ? metadata : [],
  };
}

async function attemptSwcTransform(inputCode, inputPath) {
  const label = path.basename(path.dirname(inputPath));
  console.log(`[stored fixtures] ${label}: swc`);
  const { existsSync } = fs;
  const { spawnSync } = require('child_process');
  const bin = path.join(
    repoRoot,
    'packages',
    'native-transformers',
    'target',
    'release',
    process.platform === 'win32' ? 'fixtures_cli.exe' : 'fixtures_cli'
  );

  if (!existsSync(bin)) {
    const buildEnv = { ...process.env };
    const build = spawnSync('cargo', ['build', '-p', 'fixtures_cli', '--release'], {
      cwd: path.join(repoRoot, 'packages', 'native-transformers'),
      stdio: 'inherit',
      shell: process.platform === 'win32',
      env: buildEnv,
    });
    if (build.status !== 0) {
      console.warn(`[stored fixtures] ${label}: swc build failed; skipping`);
      return { styleRules: [], success: false };
    }
  }

  const runEnv = { ...process.env };
  try {
    const babelPkg = require('@compiled/babel-plugin/package.json');
    if (babelPkg && typeof babelPkg.version === 'string' && babelPkg.version) {
      runEnv.TEST_PKG_VERSION = babelPkg.version;
    }
  } catch (_e) {
    // ignore if package cannot be resolved; transformers will fall back
  }
  const run = spawnSync(bin, [inputPath], {
    cwd: path.join(repoRoot, 'packages', 'native-transformers'),
    encoding: 'utf8',
    shell: process.platform === 'win32',
    env: runEnv,
  });

  if (run && typeof run.stderr === 'string' && run.stderr.length) {
    try {
      process.stderr.write(run.stderr);
    } catch (_e) {
      // ignore write errors
    }
  }

  if (run.status !== 0) {
    console.warn(`[stored fixtures] ${label}: swc run failed; skipping`);
    return { styleRules: [], success: false };
  }

  try {
    const parsed = JSON.parse(run.stdout || '{}');
    console.log(`[stored fixtures] ${label}: swc done`);
    return {
      styleRules: Array.isArray(parsed.styleRules) ? parsed.styleRules : [],
      success: true,
    };
  } catch (_e) {
    console.warn(`[stored fixtures] ${label}: swc output invalid; skipping`);
    return { styleRules: [], success: false };
  }
}

function canonicalRule(rule) {
  return typeof rule === 'string' ? normalizeStyleRule(rule) : JSON.stringify(rule);
}

function toArray(rules) {
  return Array.isArray(rules) ? rules : [];
}

function diffStyleRules(babelRules, swcRules) {
  const bList = toArray(babelRules);
  const sList = toArray(swcRules);
  const counts = new Map();

  for (const rule of sList) {
    const key = canonicalRule(rule);
    const entry = counts.get(key);
    if (entry) {
      entry.count += 1;
    } else {
      counts.set(key, { count: 1, value: rule });
    }
  }

  const missing = [];
  for (const rule of bList) {
    const key = canonicalRule(rule);
    const entry = counts.get(key);
    if (entry && entry.count > 0) {
      entry.count -= 1;
      if (entry.count === 0) {
        counts.delete(key);
      }
    } else {
      missing.push(rule);
    }
  }

  const extra = [];
  for (const entry of counts.values()) {
    for (let i = 0; i < entry.count; i++) {
      extra.push(entry.value);
    }
  }

  return {
    equal: missing.length === 0 && extra.length === 0 && bList.length === sList.length,
    lengthMismatch: bList.length !== sList.length,
    missing,
    extra,
    bLen: bList.length,
    sLen: sList.length,
  };
}

async function processFixture(name) {
  const fixtureDir = path.join(fixturesRoot, name);
  const inputPath = path.join(fixtureDir, 'in.jsx');
  let inputCode;
  try {
    inputCode = await fsp.readFile(inputPath, 'utf8');
  } catch (error) {
    console.warn(`[stored fixtures] ${name}: missing in.jsx (${error.message})`);
    return { name, moved: false, reason: 'missing-input' };
  }

  const startedAt = Date.now();
  console.log(`[stored fixtures] === ${name} ===`);

  let babelOutputs;
  try {
    babelOutputs = await generateBabelOutputs(fixtureDir, inputCode, inputPath);
  } catch (error) {
    console.warn(
      `[stored fixtures] ${name}: babel failed (${error && error.message ? error.message : error})`
    );
    return { name, moved: false, reason: 'babel-failed' };
  }
  const swcOutputs = await attemptSwcTransform(inputCode, inputPath);

  if (!swcOutputs.success) {
    return { name, moved: false, reason: 'swc-failed' };
  }

  await Promise.all([
    writeFileIfChanged(
      path.join(fixtureDir, 'babel-style-rules.json'),
      babelOutputs.styleRules
    ),
    writeFileIfChanged(
      path.join(fixtureDir, 'swc-style-rules.json'),
      swcOutputs.styleRules
    ),
  ]);

  const diff = diffStyleRules(babelOutputs.styleRules, swcOutputs.styleRules);

  if (diff.equal) {
    await moveFixtureToPassing(fixtureDir, name);
    console.log(
      `[stored fixtures] ${name}: moved to fixtures-passing (${(
        (Date.now() - startedAt) /
        1000
      ).toFixed(1)}s)`
    );
    return { name, moved: true };
  }

  console.warn(
    `[stored fixtures] ${name}: style-rule mismatch${
      diff.lengthMismatch ? ` length babel=${diff.bLen} swc=${diff.sLen}` : ''
    }`
  );
  if (diff.missing.length > 0) {
    console.warn(
      `  missing: ${diff.missing.slice(0, 3).map((entry) => JSON.stringify(entry)).join(', ')}`
    );
  }
  if (diff.extra.length > 0) {
    console.warn(
      `  extra: ${diff.extra.slice(0, 3).map((entry) => JSON.stringify(entry)).join(', ')}`
    );
  }
  return { name, moved: false, reason: 'style-mismatch' };
}

async function moveFixtureToPassing(sourceDir, name) {
  await fsp.mkdir(passingRoot, { recursive: true });
  const destination = path.join(passingRoot, name);
  await fsp.rm(destination, { recursive: true, force: true });
  await fsp.rename(sourceDir, destination);
}

async function main() {
  await fsp.mkdir(fixturesRoot, { recursive: true });
  await fsp.mkdir(passingRoot, { recursive: true });

  const entries = await fsp.readdir(fixturesRoot, { withFileTypes: true });
  const available = entries
    .filter((entry) => entry.isDirectory() && !entry.name.startsWith('.'))
    .map((entry) => entry.name);

  const args = process.argv.slice(2).filter(Boolean);
  let fixtures = available;
  if (args.length > 0) {
    const requested = args
      .flatMap((arg) => arg.split(','))
      .map((name) => name.trim())
      .filter(Boolean);
    const unknown = requested.filter((name) => !available.includes(name));
    if (unknown.length > 0) {
      console.error(
        `Unknown fixture name(s): ${unknown.join(', ')}\nAvailable: ${available.join(', ')}`
      );
      process.exitCode = 1;
      return;
    }
    fixtures = requested;
  }

  if (fixtures.length === 0) {
    console.log('No fixtures to process.');
    return;
  }

  const results = [];
  for (const fixture of fixtures) {
    // eslint-disable-next-line no-await-in-loop
    const result = await processFixture(fixture);
    results.push(result);
  }

  const moved = results.filter((res) => res.moved).map((res) => res.name);
  const failed = results.filter((res) => !res.moved).map((res) => res.name);

  if (moved.length > 0) {
    console.log(`Moved fixtures: ${moved.join(', ')}`);
  }
  if (failed.length > 0) {
    console.warn(`Remaining fixtures with issues: ${failed.join(', ')}`);
    process.exitCode = 1;
    return;
  }

  console.log('All processed fixtures moved to fixtures-passing.');
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
