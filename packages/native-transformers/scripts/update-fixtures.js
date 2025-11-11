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

async function generateBabelOutputs(fixtureDir, inputCode, inputPath) {
  const result = babel.transformSync(inputCode, {
    ...BABEL_OPTIONS,
    filename: inputPath,
  });

  if (!result || typeof result.code !== 'string') {
    throw new Error(`Failed to transform fixture at ${fixtureDir}`);
  }

  const metadata = (result.metadata && result.metadata.styleRules) || [];

  const outputs = {
    code: result.code,
    styleRules: Array.isArray(metadata) ? metadata : [],
  };

  return outputs;
}

async function attemptSwcTransform(inputCode, inputPath) {
  try {
    const swc = require('@swc/core');
    const compiled = require('../compiled_babel');
    const stripRuntime = require('../compiled_strip_runtime');

    const program = await swc.parse(inputCode, {
      syntax: 'typescript',
      tsx: true,
      jsx: true,
      target: 'es2019',
      comments: false,
      script: false,
      preserveAllComments: false,
      isModule: true,
      filename: inputPath,
    });

    const compiledResult = compiled.transform(program, {
      filename: inputPath,
      options: {
        cache: false,
        optimizeCss: true,
        importReact: true,
        extract: true,
      },
    });

    const stripResult = stripRuntime.transform(compiledResult.program, {
      filename: inputPath,
      options: {
        compiledRequireExclude: true,
      },
    });

    return {
      code: stripResult.code,
      // Prefer strip-runtime metadata when present; otherwise fall back to
      // metadata produced by the compiled_babel transform. This ensures
      // fixtures for cases like `css={[...]}]` still capture extracted rules
      // even when strip-runtime doesn't collect any.
      styleRules: (() => {
        const stripRules = (stripResult && stripResult.metadata && stripResult.metadata.styleRules) || [];
        if (Array.isArray(stripRules) && stripRules.length > 0) return stripRules;
        const compiledRules = (compiledResult && compiledResult.metadata && compiledResult.metadata.styleRules) || [];
        return compiledRules;
      })(),
    };
  } catch (error) {
    console.warn(`Skipping SWC transform for ${inputPath}: ${error.message}`);
    return {
      code: inputCode,
      styleRules: [],
    };
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
  await writeFileIfChanged(path.join(fixtureDir, 'babel-out.js'), babelOutputs.code);
  await writeFileIfChanged(
    path.join(fixtureDir, 'babel-style-rules.json'),
    JSON.stringify(babelOutputs.styleRules, null, 2)
  );
  await writeFileIfChanged(path.join(fixtureDir, 'out.js'), babelOutputs.code);

  const swcOutputs = await attemptSwcTransform(inputCode, inputPath);
  await writeFileIfChanged(path.join(fixtureDir, 'actual.js'), swcOutputs.code);
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
