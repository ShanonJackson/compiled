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
    const build = spawnSync('cargo', ['build', '-p', 'fixtures_cli', '--release'], {
      cwd: path.join(repoRoot, 'packages', 'native-transformers'),
      stdio: 'inherit',
      shell: process.platform === 'win32',
    });
    if (build.status !== 0) {
      console.warn(`Skipping SWC transform for ${inputPath}: failed to build fixtures_cli`);
      return { code: inputCode, styleRules: [] };
    }
  }

  let env = { ...process.env };
  try {
    const babelPkg = require('@compiled/babel-plugin/package.json');
    if (babelPkg && typeof babelPkg.version === 'string' && babelPkg.version) {
      env.TEST_PKG_VERSION = babelPkg.version;
    }
  } catch (_) {}
  const run = spawnSync(bin, [inputPath], { encoding: 'utf8', env });
  if (run.status !== 0) {
    console.warn(
      `Skipping SWC transform for ${inputPath}: fixtures_cli failed with code ${run.status}\n${run.stderr || ''}`
    );
    return { code: inputCode, styleRules: [] };
  }

  try {
    const parsed = JSON.parse(run.stdout);
    return { code: String(parsed.code || ''), styleRules: Array.isArray(parsed.styleRules) ? parsed.styleRules : [] };
  } catch (e) {
    console.warn(`Skipping SWC transform for ${inputPath}: failed to parse CLI JSON: ${e.message}`);
    return { code: inputCode, styleRules: [] };
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

  const babelOutputs = await generateBabelOutputs(fixtureDir, inputCode, inputPath);
  await writeFileIfChanged(
    path.join(fixtureDir, 'babel-out.js'),
    formatWithPrettierOrReturn(babelOutputs.code, 'babel')
  );
  await writeFileIfChanged(
    path.join(fixtureDir, 'babel-style-rules.json'),
    JSON.stringify(babelOutputs.styleRules, null, 2)
  );

  const swcOutputs = await attemptSwcTransform(inputCode, inputPath);
  // SWC output is now the canonical out.js
  await writeFileIfChanged(
    path.join(fixtureDir, 'out.js'),
    formatWithPrettierOrReturn(swcOutputs.code, 'babel')
  );
  const styleRulesToWrite =
    Array.isArray(swcOutputs.styleRules) && swcOutputs.styleRules.length > 0
      ? swcOutputs.styleRules
      : babelOutputs.styleRules;
  await writeFileIfChanged(
    path.join(fixtureDir, 'swc-style-rules.json'),
    JSON.stringify(styleRulesToWrite, null, 2)
  );
}

async function main() {
  const entries = await fsp.readdir(fixtureRoot, { withFileTypes: true });
  const fixtures = entries
    .filter((entry) => entry.isDirectory() && !entry.name.startsWith('.'))
    .map((entry) => entry.name);

  for (const fixtureName of fixtures) {
    await processFixture(fixtureName);
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
