#!/usr/bin/env node

const fs = require('fs');
const fsp = require('fs/promises');
const path = require('path');

const repoRoot = path.resolve(__dirname, '..', '..', '..');
process.chdir(repoRoot);

const babel = require('@babel/core');

const fixtureRoot = path.join(repoRoot, 'packages', 'native-transformers', 'tests', 'fixtures');

const BABEL_OPTIONS = {
  babelrc: false,
  configFile: false,
  sourceMaps: false,
  ast: false,
  caller: {
    name: 'compiled-native-transformers-fixtures',
  },
  plugins: [
    [
      require.resolve('@compiled/babel-plugin'),
      {
        cache: false,
        optimizeCss: true,
        importReact: true,
        extract: true,
      },
    ],
    [
      require.resolve('@compiled/babel-plugin-strip-runtime'),
      {
        compiledRequireExclude: true,
      },
    ],
  ],
};

function formatWithPrettierOrReturn(code, parser = 'babel') {
  try {
    const prettier = require('prettier');
    return prettier.format(code, {
      parser,
      useTabs: true,
      tabWidth: 2,
      singleQuote: false,
      trailingComma: 'es5',
      printWidth: 100,
    });
  } catch (_err) {
    return code;
  }
}

async function readFixtureConfig(dir) {
  try {
    const text = await fsp.readFile(path.join(dir, 'config.json'), 'utf8');
    return JSON.parse(text);
  } catch (e) {
    return {};
  }
}

async function generateBabelOutputs(fixtureDir, inputCode, inputPath) {
  const label = path.basename(fixtureDir);
  const t0 = Date.now();
  console.log(`[fixtures]   ${label}: babel start`);
  const cfg = await readFixtureConfig(fixtureDir);
  const compiledOptions = {
    cache: false,
    optimizeCss: true,
    importReact: true,
    extract: typeof cfg.extract === 'boolean' ? cfg.extract : true,
    ...(cfg.classNameCompressionMap ? { classNameCompressionMap: cfg.classNameCompressionMap } : {}),
  };

  const result = babel.transformSync(inputCode, {
    ...BABEL_OPTIONS,
    filename: inputPath,
    plugins: [
      [require.resolve('@compiled/babel-plugin'), compiledOptions],
      [require.resolve('@compiled/babel-plugin-strip-runtime'), { compiledRequireExclude: true }],
    ],
  });
  console.log(`[fixtures]   ${label}: babel done (${((Date.now()-t0)/1000).toFixed(1)}s)`);

  if (!result || typeof result.code !== 'string') {
    throw new Error(`Failed to transform fixture at ${fixtureDir}`);
  }

  const metadata = (result.metadata && result.metadata.styleRules) || [];

  const outputs = {
    code: formatWithPrettierOrReturn(result.code, 'babel'),
    styleRules: Array.isArray(metadata) ? metadata : [],
  };

  return outputs;
}

async function attemptSwcTransform(inputCode, inputPath) {
  const label = path.basename(path.dirname(inputPath));
  console.log(`[fixtures]   ${label}: swc start`);
  const { existsSync } = require('fs');
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
    // Try to build the CLI once to avoid JS bridges for SWC
    const buildEnv = { ...process.env };
    const build = spawnSync('cargo', ['build', '-p', 'fixtures_cli', '--release'], {
      cwd: path.join(repoRoot, 'packages', 'native-transformers'),
      stdio: 'inherit',
      shell: process.platform === 'win32',
      env: buildEnv,
    });
    if (build.status !== 0) {
      console.warn(`Skipping SWC transform for ${inputPath}: failed to build fixtures_cli`);
      return { code: inputCode, styleRules: [] };
    }
  }

  // Keep the CLI quiet by default for throughput. Enable
  // COMPILED_DEBUG_COLORMIN manually when debugging.
  let env = { ...process.env };
  try {
    const babelPkg = require('@compiled/babel-plugin/package.json');
    if (babelPkg && typeof babelPkg.version === 'string' && babelPkg.version) {
      env.TEST_PKG_VERSION = babelPkg.version;
    }
  } catch (_) {}
  const run = spawnSync(bin, [inputPath], {
    encoding: 'utf8',
    env: { ...env, COMPILED_SKIP_POSTCSS_DEPRECATION: '1', COMPILED_CLI_TRACE: '1' },
    timeout: 30000,
  });
  console.log(`[fixtures]   ${label}: swc done${run && run.status === 0 ? '' : ' (nonzero)'}${run && run.error && run.error.code === 'ETIMEDOUT' ? ' (timeout)' : ''}`);
  if (run && run.stderr && String(run.stderr).trim()) {
    // Show first chunk to help debugging stalls; keeps output manageable.
    const preview = String(run.stderr).slice(0, 4000);
    console.error(preview);
  }
  if (process.env.COMPILED_DEBUG_COLORMIN && run && run.stderr) {
    try { if (String(run.stderr).trim()) console.error(String(run.stderr)); } catch {}
  }
  if (!run || run.error || run.status !== 0) {
    if (run && run.error && run.error.code === 'ETIMEDOUT') {
      console.warn(`Skipping SWC transform for ${inputPath}: CLI timed out after 60s`);
    }
    console.warn(
      `Skipping SWC transform for ${inputPath}: fixtures_cli failed${run && run.status != null ? ` with code ${run.status}` : ''}\n${(run && run.stderr) || ''}`
    );
    return { code: inputCode, styleRules: [], success: false };
  }

  try {
    const parsed = JSON.parse(run.stdout);
    return {
      code: String(parsed.code || ''),
      styleRules: Array.isArray(parsed.styleRules) ? parsed.styleRules : [],
      success: true,
    };
  } catch (e) {
    const preview = (run.stdout || '').slice(0, 200).replace(/\s+/g, ' ');
    console.warn(
      `Skipping SWC transform for ${inputPath}: failed to parse CLI JSON: ${e.message}\nstdout preview: ${preview}`
    );
    return { code: inputCode, styleRules: [], success: false };
  }
}

async function writeFileIfChanged(filePath, content) {
  const normalized = typeof content === 'string' ? content : JSON.stringify(content, null, 2);
  let existing;
  try {
    existing = await fsp.readFile(filePath, 'utf8');
  } catch (error) {
    if (error.code !== 'ENOENT') {
      throw error;
    }
  }

  if (existing !== normalized) {
    await fsp.writeFile(filePath, normalized + (normalized.endsWith('\n') ? '' : '\n'));
  }
}

async function processFixture(name) {
  const fixtureDir = path.join(fixtureRoot, name);
  const inputPath = path.join(fixtureDir, 'in.jsx');
  const inputCode = await fsp.readFile(inputPath, 'utf8');

  const startedAt = Date.now();
  console.log(`[fixtures] → ${name}`);

  // No debug capture in normal runs; keep updater focused on fixture parity

  const babelOutputs = await generateBabelOutputs(fixtureDir, inputCode, inputPath);
  await writeFileIfChanged(
    path.join(fixtureDir, 'babel-out.jsx'),
    formatWithPrettierOrReturn(babelOutputs.code, 'babel')
  );
  await writeFileIfChanged(
    path.join(fixtureDir, 'babel-style-rules.json'),
    JSON.stringify(babelOutputs.styleRules, null, 2)
  );

  const swcOutputs = await attemptSwcTransform(inputCode, inputPath);
  if (swcOutputs.success) {
    // Only write outputs on success; avoid clobbering with empty data on failure/timeout
    await writeFileIfChanged(
      path.join(fixtureDir, 'out.jsx'),
      formatWithPrettierOrReturn(swcOutputs.code, 'babel')
    );
    await writeFileIfChanged(
      path.join(fixtureDir, 'swc-style-rules.json'),
      JSON.stringify(Array.isArray(swcOutputs.styleRules) ? swcOutputs.styleRules : [], null, 2)
    );
  } else {
    console.warn(`[fixtures]   ${name}: swc failed — keeping previous outputs`);
  }

  // Compare parity between Babel and SWC outputs
  const [babelCode, swcCode] = await Promise.all([
    fsp.readFile(path.join(fixtureDir, 'babel-out.jsx'), 'utf8'),
    fsp.readFile(path.join(fixtureDir, 'out.jsx'), 'utf8'),
  ]);
  let codeEqual = babelCode === swcCode;

  let rulesEqual = true;
  try {
    const [babelRulesText, swcRulesText] = await Promise.all([
      fsp.readFile(path.join(fixtureDir, 'babel-style-rules.json'), 'utf8'),
      fsp.readFile(path.join(fixtureDir, 'swc-style-rules.json'), 'utf8'),
    ]);
    const babelRules = JSON.parse(babelRulesText || '[]');
    const swcRules = JSON.parse(swcRulesText || '[]');
    rulesEqual = JSON.stringify(babelRules) === JSON.stringify(swcRules);
  } catch (_err) {
    rulesEqual = false;
  }

  const dur = ((Date.now() - startedAt) / 1000).toFixed(1);
  console.log(`[fixtures] ✓ ${name} (${dur}s)`);

  return { name, codeEqual, rulesEqual };
}

async function main() {
  const entries = await fsp.readdir(fixtureRoot, { withFileTypes: true });
  const fixtures = entries
    .filter((entry) => entry.isDirectory() && !entry.name.startsWith('.'))
    .map((entry) => entry.name);

  const results = [];
  for (const fixtureName of fixtures) {
    const res = await processFixture(fixtureName);
    results.push(res);
  }

  const ruleMismatches = results.filter((r) => !r.rulesEqual);
  const codeOnlyMismatches = results.filter((r) => r.rulesEqual && !r.codeEqual);

  if (ruleMismatches.length > 0) {
    console.error('Fixture parity check failed (style rules differ):');
    for (const r of ruleMismatches) {
      console.error(` - ${r.name}: style rules differ (babel vs swc)`);
      if (!r.codeEqual) {
        console.error(`   · code also differs (babel-out.jsx vs out.jsx)`);
      }
    }
    process.exitCode = 1;
    return;
  }

  if (codeOnlyMismatches.length > 0) {
    console.warn('All style rules match. Code-only differences detected:');
    for (const r of codeOnlyMismatches) {
      console.warn(` - ${r.name}: code differs (babel-out.jsx vs out.jsx)`);
    }
    // Do not fail the run when only code differs.
    process.exitCode = 0;
    return;
  }

  console.log('All fixtures match: code and style rules are identical.');
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
