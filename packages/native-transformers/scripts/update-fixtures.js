#!/usr/bin/env node

const fs = require('fs');
const fsp = require('fs/promises');
const path = require('path');
const os = require('os');

const repoRoot = path.resolve(__dirname, '..', '..', '..');
const workspaceNodeModules = path.join(repoRoot, 'node_modules');
// Resolver alignment is enabled by default to mirror the collectors; set
// COMPILED_FIXTURES_ENABLE_RESOLVER=0 to disable.
const ENABLE_RESOLVER = false;
// Always align CWD with the standalone workspace.
process.chdir(repoRoot);

const workspaceBrowserslistConfig = path.join(repoRoot, '.browserslistrc');
if (ENABLE_RESOLVER && fs.existsSync(workspaceBrowserslistConfig)) {
  process.env.BROWSERSLIST_CONFIG = workspaceBrowserslistConfig;
}

const resolveFromRepo = (id) =>
  require.resolve(id, { paths: [workspaceNodeModules, repoRoot] });
const requireFromRepo = (id) => require(resolveFromRepo(id));
const babel = requireFromRepo('@babel/core');
const compiledConfigPath =
  process.env.COMPILED_FIXTURES_CONFIG || path.join(repoRoot, '.compiledcssrc');
let cachedCompiledConfig = null;
let cachedTokensOptions = null;

const fixtureRoot = path.join(
  repoRoot,
  'packages',
  'native-transformers',
  'tests',
  'fixtures'
);

const MAX_MISMATCH_PRINT = 3; // show at most this many missing/extra rules
const timingTotals = { totalMs: 0, postcssMs: 0, postcssPluginMs: 0, samples: 0 };

const BABEL_OPTIONS = {
  babelrc: false,
  configFile: false,
  parserOpts: {
    sourceType: 'module',
    plugins: ['typescript', 'jsx'],
  },
  sourceMaps: false,
  ast: false,
  caller: { name: 'compiled-native-transformers-fixtures' },
};

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

function normalizeMediaRule(rule) {
  const open = rule.indexOf('{');
  if (open === -1) return rule.trim();
  let depth = 0;
  let close = -1;
  for (let i = open; i < rule.length; i++) {
    const ch = rule[i];
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
    return rule.trim();
  }
  const prefix = rule.slice(0, open + 1);
  const body = rule.slice(open + 1, close);
  const suffix = rule.slice(close);
  const segments = splitTopLevelSegments(body)
    .map((segment) => normalizeStyleRule(segment.trim()))
    .filter(Boolean);
  if (segments.length <= 1) {
    return rule.trim();
  }
  segments.sort();
  return `${prefix}${segments.join('')}${suffix}`.trim();
}

function normalizeStyleRule(rule) {
  const trimmed = (rule || '').trim();
  if (trimmed.startsWith('@media')) {
    return normalizeMediaRule(trimmed);
  }
  return trimmed;
}

function formatWithPrettierOrReturn(code, parser = 'babel-ts') {
  if (typeof code !== 'string') {
    return code;
  }
  // Formatting is intentionally skipped to avoid Prettier parser/plugin crashes on generated code.
  return code;
}

function loadTokensPlugin() {
  const candidates = [
    () => requireFromRepo('@atlaskit/tokens/babel-plugin'),
  ];
  for (const load of candidates) {
    try {
      return load();
    } catch (_e) {
      // try next
    }
  }
  return null;
}

function loadCompiledConfig() {
  if (cachedCompiledConfig !== null) {
    return cachedCompiledConfig;
  }
  try {
    const raw = fs.readFileSync(compiledConfigPath, 'utf8');
    cachedCompiledConfig = JSON.parse(raw);
  } catch (_e) {
    cachedCompiledConfig = {};
  }
  return cachedCompiledConfig;
}

function getResolverOptions() {
  const cfg = loadCompiledConfig();
  const envResolver = process.env.COMPILED_FIXTURES_RESOLVER;
  const envResolverCompat = process.env.COMPILED_FIXTURES_RESOLVER_COMPAT;

  const parseCompat = (value) => {
    if (!value) return undefined;
    try {
      return JSON.parse(value);
    } catch (_e) {
      return undefined;
    }
  };

  let resolver = envResolver || (cfg && typeof cfg === 'object' ? cfg.resolver : undefined);
  if (resolver) {
    try {
      resolver = resolveFromRepo(resolver);
    } catch (_e) {
      // leave resolver as-is when it cannot be resolved locally
    }
  }

  const resolverCompat =
    parseCompat(envResolverCompat) ||
    (cfg && typeof cfg === 'object' ? cfg.resolverCompat : undefined);

  return { resolver, resolverCompat };
}

function getTokensPrepassOptions() {
  if (cachedTokensOptions !== null) {
    return cachedTokensOptions;
  }
  const config = loadCompiledConfig();
  const plugins = Array.isArray(config.transformerBabelPlugins)
    ? config.transformerBabelPlugins
    : [];
  let options = {};
  for (const entry of plugins) {
    if (entry === '@atlaskit/tokens/babel-plugin') {
      options = {};
      break;
    }
    if (Array.isArray(entry) && entry[0] === '@atlaskit/tokens/babel-plugin') {
      const opts = entry[1];
      if (opts && typeof opts === 'object') {
        options = opts;
      }
      break;
    }
  }
  cachedTokensOptions = { shouldInsertStyles: false, ...options };
  return cachedTokensOptions;
}

function shouldPrepassTokens(inputCode) {
  return (
    inputCode.includes('@atlaskit/tokens') ||
    /\btoken\(['"]/.test(inputCode) ||
    /\bdefineToken\(['"]/.test(inputCode)
  );
}

function applyTokensPrepass(code, filename) {
  const tokensPlugin = loadTokensPlugin();
  if (!shouldPrepassTokens(code)) {
    return code;
  }
  try {
    if (!tokensPlugin) {
      throw new Error('no tokens plugin');
    }
    const result = babel.transformSync(code, {
      filename,
      babelrc: false,
      configFile: false,
      ast: false,
      code: true,
      parserOpts: {
        sourceType: 'module',
        plugins: ['typescript', 'jsx'],
      },
      plugins: [[tokensPlugin, getTokensPrepassOptions()]],
      envName: 'test',
    });
    return (result && result.code) || code;
  } catch (_err) {
    // If tokens prepass fails (e.g., missing token or plugin resolution), fall back to original code.
    return code;
  }
}

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
    await fsp.writeFile(filePath, normalized + (normalized.endsWith('\n') ? '' : '\n'));
  }
}

async function generateBabelOutputs(fixtureDir, inputCode, inputPath) {
  const label = path.basename(fixtureDir);
  const t0 = Date.now();
  const cfg = await readFixtureConfig(fixtureDir);
  const tokenized = applyTokensPrepass(inputCode, inputPath);
  const { resolver } = ENABLE_RESOLVER ? getResolverOptions() : {};
  const compiledOptions = {
    cache: false,
    optimizeCss: true,
    importReact: true,
    extract: typeof cfg.extract === 'boolean' ? cfg.extract : true,
    ...(cfg.classNameCompressionMap
      ? { classNameCompressionMap: cfg.classNameCompressionMap }
      : {}),
    ...(resolver ? { resolver } : {}),
  };

  const result = babel.transformSync(tokenized, {
    ...BABEL_OPTIONS,
    filename: inputPath,
    plugins: [
      [resolveFromRepo('@compiled/babel-plugin'), compiledOptions],
      [resolveFromRepo('@compiled/babel-plugin-strip-runtime'), { compiledRequireExclude: true }],
    ],
  });

  if (!result || typeof result.code !== 'string') {
    throw new Error(`Failed to transform fixture at ${fixtureDir}`);
  }

  const metadata = (result.metadata && result.metadata.styleRules) || [];
  return {
    code: formatWithPrettierOrReturn(result.code, 'babel-ts'),
    styleRules: Array.isArray(metadata) ? metadata : [],
  };
}

async function attemptSwcTransform(inputCode, inputPath, fixtureConfig = {}) {
  const label = path.basename(path.dirname(inputPath));
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
    const buildEnv = { ...process.env };
    const build = spawnSync('cargo', ['build', '-p', 'fixtures_cli', '--release'], {
      cwd: path.join(repoRoot, 'packages', 'native-transformers'),
      stdio: 'inherit',
      shell: process.platform === 'win32',
      env: buildEnv,
    });
    if (build.status !== 0) {
      console.warn(`[fixtures] ${label}: swc build failed; skipping`);
      return { code: inputCode, styleRules: [], success: false };
    }
  }

  // Pre-pass with tokens plugin to mirror Jira collector behavior
  const tokenized = applyTokensPrepass(inputCode, inputPath);

  // Keep the tokenized file alongside the original so relative imports resolve correctly.
  const tmpFileBase = `.tmp-swc-${label}-${Date.now()}-${Math.random().toString(16).slice(2)}`;
  const tmpFile = path.join(
    path.dirname(inputPath),
    `${tmpFileBase}${path.extname(inputPath)}`
  );
  await fsp.writeFile(tmpFile, tokenized, 'utf8');

  const runEnv = { ...process.env };
  if (ENABLE_RESOLVER && fs.existsSync(workspaceBrowserslistConfig)) {
    runEnv.BROWSERSLIST_CONFIG = workspaceBrowserslistConfig;
  }
  const { resolver, resolverCompat } = ENABLE_RESOLVER ? getResolverOptions() : {};
  if (resolver) {
    runEnv.COMPILED_FIXTURES_RESOLVER =
      typeof resolver === 'string' ? resolver : JSON.stringify(resolver);
  }
  if (resolverCompat) {
    runEnv.COMPILED_FIXTURES_RESOLVER_COMPAT = JSON.stringify(resolverCompat);
  }
  // Ensure native transformer headers report the same version as Babel.
  // The Rust transformers read TEST_PKG_VERSION to embed the plugin version.
  try {
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const babelPkg = require('@compiled/babel-plugin/package.json');
    if (babelPkg && typeof babelPkg.version === 'string' && babelPkg.version) {
      runEnv.TEST_PKG_VERSION = babelPkg.version;
    }
  } catch (_e) {
    // ignore if package cannot be resolved; transformers will fall back
  }
  const runCwd = path.join(repoRoot, 'packages', 'native-transformers');
  const run = spawnSync(bin, [tmpFile], {
    cwd: runCwd,
    encoding: 'utf8',
    shell: process.platform === 'win32',
    env: runEnv,
  });

  // Clean up temp file
  try {
    await fsp.unlink(tmpFile);
  } catch (_e) {
    // ignore
  }

  // Forward stderr so trace logs are visible when COMPILED_CLI_TRACE=1
  if (run && typeof run.stderr === 'string' && run.stderr.length) {
    try {
      process.stderr.write(run.stderr);
    } catch (_e) {
      // ignore write errors
    }
  }

  if (run.status !== 0) {
    console.warn(`[fixtures] ${label}: swc run failed; skipping`);
    return { code: inputCode, styleRules: [], success: false };
  }

  try {
    const parsed = JSON.parse(run.stdout || '{}');
    if (parsed && parsed.timings) {
      timingTotals.samples += 1;
      timingTotals.totalMs += Number(parsed.timings.totalMs) || 0;
      timingTotals.postcssMs += Number(parsed.timings.postcssMs) || 0;
      timingTotals.postcssPluginMs += Number(parsed.timings.postcssPluginMs) || 0;
    }
    return {
      code: parsed.code || inputCode,
      styleRules: Array.isArray(parsed.styleRules) ? parsed.styleRules : [],
      success: true,
    };
  } catch (_e) {
    console.warn(`[fixtures] ${label}: swc output invalid; skipping`);
    return { code: inputCode, styleRules: [], success: false };
  }
}

async function processFixture(name) {
  const fixtureDir = path.join(fixtureRoot, name);
  // Prefer TSX, then JSX
  const inputPathTsx = path.join(fixtureDir, 'in.tsx');
  const inputPathJsx = path.join(fixtureDir, 'in.jsx');
  const inputPath = fs.existsSync(inputPathTsx) ? inputPathTsx : inputPathJsx;
  const inputCode = await fsp.readFile(inputPath, 'utf8');

  const cfg = await readFixtureConfig(fixtureDir);

  const startedAt = Date.now();

  const babelOutputs = await generateBabelOutputs(fixtureDir, inputCode, inputPath, cfg);
  await writeFileIfChanged(path.join(fixtureDir, 'babel-out.jsx'), babelOutputs.code);
  await writeFileIfChanged(
    path.join(fixtureDir, 'babel-style-rules.json'),
    JSON.stringify(babelOutputs.styleRules, null, 2)
  );

  const swcOutputs = await attemptSwcTransform(inputCode, inputPath, cfg);
  if (swcOutputs.success) {
    await writeFileIfChanged(path.join(fixtureDir, 'out.jsx'), swcOutputs.code);
    await writeFileIfChanged(
      path.join(fixtureDir, 'swc-style-rules.json'),
      JSON.stringify(
        Array.isArray(swcOutputs.styleRules) ? swcOutputs.styleRules : [],
        null,
        2
      )
    );
  } else {
    console.warn(`[fixtures] ${name}: swc failed; keeping previous outputs`);
  }

  const [babelCode, swcCode] = await Promise.all([
    fsp.readFile(path.join(fixtureDir, 'babel-out.jsx'), 'utf8'),
    fsp.readFile(path.join(fixtureDir, 'out.jsx'), 'utf8'),
  ]);
  // We don't gate on code equality; style-rules are the primary parity target.
  const codeEqual = true;

  // Compare style-rules ignoring order
  let rulesEqual = true;
  let ruleReport = null;
  try {
    const [babelRulesText, swcRulesText] = await Promise.all([
      fsp.readFile(path.join(fixtureDir, 'babel-style-rules.json'), 'utf8'),
      fsp.readFile(path.join(fixtureDir, 'swc-style-rules.json'), 'utf8'),
    ]);
    const babelRules = JSON.parse(babelRulesText || '[]');
    const swcRules = JSON.parse(swcRulesText || '[]');
    const normalizedBabel = babelRules.map(normalizeStyleRule);
    const normalizedSwc = swcRules.map(normalizeStyleRule);

    const bSet = new Set(normalizedBabel);
    const sSet = new Set(normalizedSwc);
    const missing = [];
    const extra = [];
    normalizedBabel.forEach((rule, idx) => {
      if (!sSet.has(rule)) {
        missing.push(babelRules[idx]);
      }
    });
    normalizedSwc.forEach((rule, idx) => {
      if (!bSet.has(rule)) {
        extra.push(swcRules[idx]);
      }
    });

    const lengthMismatch = babelRules.length !== swcRules.length;
    rulesEqual = missing.length === 0 && extra.length === 0;

    if (!rulesEqual || lengthMismatch) {
      ruleReport = {
        lengthMismatch,
        bLen: babelRules.length,
        sLen: swcRules.length,
        missing: missing.slice(0, MAX_MISMATCH_PRINT),
        extra: extra.slice(0, MAX_MISMATCH_PRINT),
      };
    }
  } catch (_err) {
    rulesEqual = false;
    ruleReport = { error: 'failed to read/parse style-rules' };
  }

  const dur = ((Date.now() - startedAt) / 1000).toFixed(1);

  return { name, codeEqual, rulesEqual, ruleReport };
}

async function main() {
  const entries = await fsp.readdir(fixtureRoot, { withFileTypes: true });
  const allFixtures = entries
    .filter((e) => e.isDirectory() && !e.name.startsWith('.'))
    .map((e) => e.name);

  const args = process.argv.slice(2).filter(Boolean);
  let fixtures = allFixtures;
  if (args.length > 0) {
    const requested = args
      .flatMap((a) => a.split(','))
      .map((s) => s.trim())
      .filter(Boolean);
    const unknown = requested.filter((n) => !allFixtures.includes(n));
    if (unknown.length > 0) {
      console.error(
        `Unknown fixture name(s): ${unknown.join(', ')}\nAvailable: ${allFixtures.join(', ')}`
      );
      process.exitCode = 1;
      return;
    }
    fixtures = requested;
  }

  const results = [];
  for (const fixtureName of fixtures) {
    const res = await processFixture(fixtureName);
    results.push(res);
  }

  if (timingTotals.samples > 0) {
    const pct =
      timingTotals.totalMs > 0 ? (timingTotals.postcssMs / timingTotals.totalMs) * 100 : 0;
    const pluginPct =
      timingTotals.postcssMs > 0
        ? (timingTotals.postcssPluginMs / timingTotals.postcssMs) * 100
        : 0;
    console.log(
      `Timing (SWC fixtures): total ${timingTotals.totalMs.toFixed(
        1
      )}ms, postcss ${timingTotals.postcssMs.toFixed(1)}ms (${pct.toFixed(
        1
      )}%) plugin ${timingTotals.postcssPluginMs.toFixed(1)}ms (${pluginPct.toFixed(
        1
      )}% of postcss) across ${timingTotals.samples} fixtures`
    );
  }

  const ruleMismatches = results.filter(
    (r) => !r.rulesEqual || (r.ruleReport && r.ruleReport.lengthMismatch)
  );
  const codeOnlyMismatches = results.filter(
    (r) => (r.rulesEqual || ruleMismatches.length === 0) && !r.codeEqual
  );

  if (ruleMismatches.length > 0) {
    console.error('Style-rules mismatches (order ignored):');
    for (const r of ruleMismatches) {
      const rep = r.ruleReport || {};
      const parts = [];
      if (rep.lengthMismatch)
        parts.push(`length mismatch babel=${rep.bLen} swc=${rep.sLen}`);
      if (rep.missing && rep.missing.length)
        parts.push(`missing: ${rep.missing.map((s) => JSON.stringify(s)).join(', ')}`);
      if (rep.extra && rep.extra.length)
        parts.push(`extra: ${rep.extra.map((s) => JSON.stringify(s)).join(', ')}`);
      if (rep.error) parts.push(rep.error);
      console.error(` - ${r.name}: ${parts.join(' | ')}`);
      if (!r.codeEqual) console.error(`   code also differs`);
    }
    process.exitCode = 1;
    return;
  }

  if (codeOnlyMismatches.length > 0) {
    console.warn('Code-only differences:');
    for (const r of codeOnlyMismatches) {
      console.warn(` - ${r.name}: babel-out.jsx vs out.jsx`);
    }
    process.exitCode = 0;
    return;
  }

  console.log('All fixtures match.');
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
