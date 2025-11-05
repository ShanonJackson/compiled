#!/usr/bin/env node

/**
 * Local helper for comparing compiled style rules without mutating the shared jira repo.
 * Usage: node scripts/analyze-style-rules-local.js --babel <path> --swc <path>
 * When omitted, defaults to the jira tmp/style-rules/{babel,swc} directories.
 */

const fs = require('fs/promises');
const path = require('path');

const DEFAULT_ROOT = path.resolve(__dirname, '../../../jira/tmp/style-rules');
const DEFAULT_BABEL = path.join(DEFAULT_ROOT, 'babel');
const DEFAULT_SWC = path.join(DEFAULT_ROOT, 'swc');
const INCLUDED_PATH_PREFIXES = ['src/packages/'];

function parseArgs() {
  const result = {
    babel: DEFAULT_BABEL,
    swc: DEFAULT_SWC,
    swcOverlay: null,
  };

  const argv = process.argv.slice(2);
  for (let index = 0; index < argv.length; index += 1) {
    const current = argv[index];
    if ((current === '--babel' || current === '--swc') && index + 1 < argv.length) {
      result[current.slice(2)] = path.resolve(argv[index + 1]);
      index += 1;
    } else if (current === '--swc-overlay' && index + 1 < argv.length) {
      result.swcOverlay = path.resolve(argv[index + 1]);
      index += 1;
    } else if (current === '--help' || current === '-h') {
      console.log(
        'Usage: node analyze-style-rules-local.js [--babel <dir>] [--swc <dir>] [--swc-overlay <dir>]\n\n' +
          'Defaults to jira/tmp/style-rules/{babel,swc}. When --swc-overlay is provided, files in that directory override matching files from --swc.'
      );
      process.exit(0);
    }
  }

  return result;
}

function shouldInclude(relativePath) {
  if (!INCLUDED_PATH_PREFIXES.length) {
    return true;
  }

  return INCLUDED_PATH_PREFIXES.some((prefix) => relativePath.startsWith(prefix));
}

async function assertDirectoryExists(name, absolutePath) {
  try {
    const stats = await fs.stat(absolutePath);
    if (!stats.isDirectory()) {
      throw new Error(`Expected '${absolutePath}' to be a directory`);
    }
  } catch (error) {
    if (error && error.code === 'ENOENT') {
      throw new Error(`Missing '${name}' directory at '${absolutePath}'`);
    }
    throw error;
  }
}

async function walkDirectory(rootDirectory) {
  const result = [];
  const queue = [rootDirectory];

  while (queue.length) {
    const current = queue.pop();
    const entries = await fs.readdir(current, { withFileTypes: true });

    for (const entry of entries) {
      const absolutePath = path.join(current, entry.name);
      if (entry.isDirectory()) {
        queue.push(absolutePath);
        continue;
      }

      if (entry.isFile() && entry.name.endsWith('.json')) {
        result.push(absolutePath);
      }
    }
  }

  return result;
}

function prepareStyleRules(rawRules, label, relativePath) {
  if (!Array.isArray(rawRules)) {
    throw new Error(`Found non-array styleRules for '${relativePath}' in '${label}'`);
  }

  return {
    raw: Array.from(new Set(rawRules)).sort(),
  };
}

async function buildStyleRuleMap(directoryPath, label, overlayDirectory) {
  const files = await walkDirectory(directoryPath);
  const overlayFiles = overlayDirectory ? await walkDirectory(overlayDirectory) : [];
  const entries = new Map();
  const empty = new Set();
  const allFiles = new Set();
  const missingCandidates = new Map();
  const fileSources = new Map();

  for (const filePath of files) {
    const relativePath = path.relative(directoryPath, filePath);
    const normalizedPath = relativePath.split(path.sep).join('/');
    if (!shouldInclude(normalizedPath)) {
      continue;
    }
    fileSources.set(normalizedPath, { absolutePath: filePath, source: label });
  }

  for (const overlayPath of overlayFiles) {
    const relativePath = path.relative(overlayDirectory, overlayPath);
    const normalizedPath = relativePath.split(path.sep).join('/');
    if (!shouldInclude(normalizedPath)) {
      continue;
    }
    fileSources.set(normalizedPath, {
      absolutePath: overlayPath,
      source: `${label} (overlay)`,
    });
  }

  for (const [normalizedPath, { absolutePath, source }] of fileSources) {
    const relativePath =
      source === label
        ? path.relative(directoryPath, absolutePath)
        : path.relative(overlayDirectory, absolutePath);

    if (!shouldInclude(normalizedPath)) {
      continue;
    }

    allFiles.add(normalizedPath);
    let parsed;

    try {
      const contents = await fs.readFile(absolutePath, 'utf8');
      parsed = JSON.parse(contents);
    } catch (error) {
      throw new Error(
        `Unable to parse JSON for '${relativePath}' in '${source}': ${error.message}`
      );
    }

    if (!Array.isArray(parsed.styleRules)) {
      throw new Error(`Found non-array styleRules for '${relativePath}' in '${label}'`);
    }

    const prepared = prepareStyleRules(parsed.styleRules, label, relativePath);

    if (prepared.raw.length === 0) {
      empty.add(normalizedPath);
      missingCandidates.set(normalizedPath, 0);
      continue;
    }

    missingCandidates.set(normalizedPath, prepared.raw.length);
    entries.set(normalizedPath, prepared);
  }

  return { entries, empty, files: allFiles, sizes: missingCandidates };
}

function diffStyleRuleMaps(left, right, leftLabel, rightLabel) {
  const keys = new Set([...left.files, ...right.files]);
  const sortedKeys = Array.from(keys).sort();
  let matchedCount = 0;
  let processedCount = 0;

  for (const key of sortedKeys) {
    processedCount += 1;

    const leftEntry = left.entries.get(key);
    const rightEntry = right.entries.get(key);
    const leftEmpty = left.empty.has(key);
    const rightEmpty = right.empty.has(key);

    if (!leftEntry && !leftEmpty) {
      const leftSize = left.sizes.get(key) || 0;
      if (!rightEntry && rightEmpty) {
        continue;
      }
      if (leftSize === 0) {
        continue;
      }
      return {
        matchedCount,
        remainingCount: sortedKeys.length - processedCount,
        error: `File '${key}' is missing in '${leftLabel}'`,
      };
    }

    if (!rightEntry && !rightEmpty) {
      const rightSize = right.sizes.get(key) || 0;
      if (!leftEntry && leftEmpty) {
        continue;
      }
      if (rightSize === 0) {
        continue;
      }
      return {
        matchedCount,
        remainingCount: sortedKeys.length - processedCount,
        error: `File '${key}' is missing in '${rightLabel}'`,
      };
    }

    if (leftEmpty && rightEntry && !rightEmpty) {
      return {
        matchedCount,
        remainingCount: sortedKeys.length - processedCount,
        error: `File '${key}' has style rules in '${rightLabel}' but is empty in '${leftLabel}'`,
      };
    }

    if (rightEmpty && leftEntry && !leftEmpty) {
      return {
        matchedCount,
        remainingCount: sortedKeys.length - processedCount,
        error: `File '${key}' has style rules in '${leftLabel}' but is empty in '${rightLabel}'`,
      };
    }

    if (!leftEntry || !rightEntry) {
      continue;
    }

    const leftRules = leftEntry.raw;
    const rightRules = rightEntry.raw;

    if (leftRules.length !== rightRules.length) {
      return {
        matchedCount,
        remainingCount: sortedKeys.length - processedCount,
        error: `Style rule count differs for '${key}' (${leftLabel}: ${leftRules.length}, ${rightLabel}: ${rightRules.length})`,
      };
    }

    for (let index = 0; index < leftRules.length; index += 1) {
      if (leftRules[index] !== rightRules[index]) {
        return {
          matchedCount,
          remainingCount: sortedKeys.length - processedCount,
          error: `Style rules differ for '${key}'. First mismatch at index ${index}: '${leftLabel}' has '${leftRules[index] || '(missing)'}', '${rightLabel}' has '${rightRules[index] || '(missing)'}'`,
        };
      }
    }

    matchedCount += 1;
  }

  return { matchedCount, remainingCount: 0, error: null };
}

async function main() {
  const { babel, swc, swcOverlay } = parseArgs();

  await Promise.all([
    assertDirectoryExists('babel', babel),
    assertDirectoryExists('swc', swc),
  ]);

  const [babelMap, swcMap] = await Promise.all([
    buildStyleRuleMap(babel, 'babel'),
    buildStyleRuleMap(swc, 'swc', swcOverlay),
  ]);

  const { matchedCount, remainingCount, error } = diffStyleRuleMaps(
    babelMap,
    swcMap,
    'babel',
    'swc',
  );

  if (error) {
    console.error(`Matched files: ${matchedCount}`);
    console.error(`Remaining files (approx): ${remainingCount}`);
    console.error(error);
    process.exitCode = 1;
    return;
  }

  console.log(`Style rules match for ${matchedCount} files.`);
}

main().catch((error) => {
  console.error(error.message || error);
  process.exitCode = 1;
});
