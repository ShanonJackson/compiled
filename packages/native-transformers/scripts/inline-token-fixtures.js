#!/usr/bin/env node
'use strict';

const fsp = require('fs/promises');
const path = require('path');

const parser = require('@babel/parser');
const traverse = require('@babel/traverse').default;
const generate = require('@babel/generator').default;
const t = require('@babel/types');

const tokenNames = require('@atlaskit/tokens/dist/cjs/artifacts/token-names').default;
const legacyLightTokens =
  require('@atlaskit/tokens/dist/cjs/artifacts/tokens-raw/atlassian-legacy-light').default;
const lightTokens =
  require('@atlaskit/tokens/dist/cjs/artifacts/tokens-raw/atlassian-light').default;
const shapeTokens =
  require('@atlaskit/tokens/dist/cjs/artifacts/tokens-raw/atlassian-shape').default;
const spacingTokens =
  require('@atlaskit/tokens/dist/cjs/artifacts/tokens-raw/atlassian-spacing').default;
const typographyTokens =
  require('@atlaskit/tokens/dist/cjs/artifacts/tokens-raw/atlassian-typography-adg3').default;

const repoRoot = path.resolve(__dirname, '..', '..', '..');
const fixturesRoot = path.join(
  repoRoot,
  'packages',
  'native-transformers',
  'tests',
  'stored',
  'fixtures'
);

const parserOptions = {
  sourceType: 'module',
  sourceFilename: undefined,
  plugins: [
    'jsx',
    'typescript',
    'classProperties',
    'classPrivateProperties',
    'classPrivateMethods',
    'dynamicImport',
    'nullishCoalescingOperator',
    'optionalChaining',
    'objectRestSpread',
    'topLevelAwait',
  ],
  allowAwaitOutsideFunction: true,
  ranges: true,
};

const generatorOptions = {
  retainLines: true,
  compact: false,
  jsonCompatibleStrings: false,
  quotes: 'single',
  jsescOption: {
    quotes: 'single',
    minimal: true,
  },
};

const pluginOptions = {
  defaultTheme: 'light',
};

const FORCE_AUTO_FALLBACK_BASE_EXEMPTIONS = ['radius'];
const lightValues = getThemeValues(lightTokens);
const legacyLightValues = getThemeValues(legacyLightTokens);
const shapeValues = getThemeValues(shapeTokens);
const spacingValues = getThemeValues(spacingTokens);
const typographyValues = getThemeValues(typographyTokens);

async function main() {
  const fixtureInputs = await collectFixtureInputs(fixturesRoot);
  if (!fixtureInputs.length) {
    console.log('[tokens-inline] No fixture inputs found.');
    return;
  }

  let updated = 0;
  for (const filePath of fixtureInputs.sort()) {
    const code = await fsp.readFile(filePath, 'utf8');
    if (!code.includes('token(')) {
      continue;
    }

    const result = transformSource(code, filePath);
    if (!result || result === code) {
      continue;
    }

    await fsp.writeFile(filePath, result, 'utf8');
    updated++;
    console.log(`[tokens-inline] updated ${path.relative(repoRoot, filePath)}`);
  }

  console.log(`[tokens-inline] Completed. ${updated} file(s) updated.`);
}

function transformSource(source, filename) {
  let ast;
  try {
    ast = parser.parse(source, { ...parserOptions, sourceFilename: filename });
  } catch (error) {
    throw new Error(`[tokens-inline] Failed to parse ${filename}: ${error.message}`);
  }

  const replacements = [];
  const bindingRecords = new Map();

  traverse(ast, {
    CallExpression(path) {
      const binding = getTokenBinding(path);
      if (!binding) {
        return;
      }

      const replacementNode = buildReplacementNode(path.node, pluginOptions);
      if (!replacementNode) {
        return;
      }

      replacements.push({
        start: path.node.start,
        end: path.node.end,
        text: generate(replacementNode, generatorOptions).code,
      });

      const record = getBindingRecord(bindingRecords, binding);
      record.replacedRefs += 1;
    },
  });

  if (!replacements.length) {
    return null;
  }

  const importReplacements = buildImportReplacements(bindingRecords);
  const output = applyReplacements(source, replacements.concat(importReplacements));
  return output === source ? null : output;
}

function getBindingRecord(bindingRecords, binding) {
  const key = `${binding.identifier.name}:${binding.identifier.start}:${binding.identifier.end}`;
  let record = bindingRecords.get(key);

  if (!record) {
    const importPath = binding.path.parentPath;
    if (!importPath || !t.isImportDeclaration(importPath.node)) {
      throw new Error('[tokens-inline] Token binding missing import declaration.');
    }

    record = {
      key,
      localName: binding.identifier.name,
      importNode: importPath.node,
      importStart: importPath.node.start,
      importEnd: importPath.node.end,
      totalRefs: binding.referencePaths.length,
      replacedRefs: 0,
    };
    bindingRecords.set(key, record);
  }

  return record;
}

function buildImportReplacements(bindingRecords) {
  const replacements = [];
  const importChanges = new Map();

  for (const record of bindingRecords.values()) {
    if (record.replacedRefs === 0 || record.replacedRefs !== record.totalRefs) {
      continue;
    }

    const key = `${record.importStart}:${record.importEnd}`;
    let change = importChanges.get(key);

    if (!change) {
      change = {
        start: record.importStart,
        end: record.importEnd,
        node: t.cloneNode(record.importNode, true),
        removeLocalNames: new Set(),
      };
      importChanges.set(key, change);
    }

    change.removeLocalNames.add(record.localName);
  }

  for (const change of importChanges.values()) {
    change.node.specifiers = change.node.specifiers.filter((specifier) => {
      if (!t.isImportSpecifier(specifier)) {
        return true;
      }
      if (getNonAliasedImportName(specifier) !== 'token') {
        return true;
      }
      return !change.removeLocalNames.has(specifier.local.name);
    });

    const text =
      change.node.specifiers.length === 0
        ? ''
        : generate(change.node, generatorOptions).code;
    replacements.push({ start: change.start, end: change.end, text });
  }

  return replacements;
}

function applyReplacements(source, replacements) {
  if (!replacements.length) {
    return source;
  }

  let result = source;
  replacements
    .slice()
    .sort((a, b) => b.start - a.start)
    .forEach((replacement) => {
      result =
        result.slice(0, replacement.start) +
        replacement.text +
        result.slice(replacement.end);
    });

  return result;
}

function buildReplacementNode(callNode, options) {
  if (!callNode.arguments[0]) {
    throw new Error('token() requires at least one argument');
  }
  const firstArg = callNode.arguments[0];
  if (!t.isStringLiteral(firstArg)) {
    throw new Error('token() must have a string literal as the first argument');
  }
  if (callNode.arguments.length > 2) {
    throw new Error(`token() does not accept ${callNode.arguments.length} arguments`);
  }

  const tokenName = firstArg.value;
  const cssTokenValue = tokenNames[tokenName];
  if (!cssTokenValue) {
    throw new Error(`token '${tokenName}' does not exist`);
  }

  let replacementNode = null;
  const defaultTheme = options.defaultTheme || 'light';

  if (callNode.arguments.length < 2) {
    if (options.shouldUseAutoFallback !== false) {
      replacementNode = t.stringLiteral(
        `var(${cssTokenValue}, ${getDefaultFallback(tokenName, defaultTheme)})`
      );
    } else {
      replacementNode = t.stringLiteral(`var(${cssTokenValue})`);
    }
  }

  const forceExemptions = FORCE_AUTO_FALLBACK_BASE_EXEMPTIONS.concat(
    options.forceAutoFallbackExemptions || []
  );
  const fallbackNode =
    options.shouldForceAutoFallback !== false && !isExempted(tokenName, forceExemptions)
      ? t.stringLiteral(getDefaultFallback(tokenName, defaultTheme))
      : callNode.arguments[1];

  if (t.isStringLiteral(fallbackNode)) {
    replacementNode = t.stringLiteral(
      fallbackNode.value ? `var(${cssTokenValue}, ${fallbackNode.value})` : `var(${cssTokenValue})`
    );
  } else if (fallbackNode && t.isExpression(fallbackNode)) {
    replacementNode = t.templateLiteral(
      [
        t.templateElement(
          {
            cooked: `var(${cssTokenValue}, `,
            raw: `var(${escapeForTemplateLiteral(cssTokenValue)}, `,
          },
          false
        ),
        t.templateElement({ cooked: ')', raw: ')' }, true),
      ],
      [fallbackNode]
    );
  }

  return replacementNode;
}

function getTokenBinding(path) {
  const callee = path.node.callee;
  if (!t.isIdentifier(callee)) {
    return null;
  }

  const binding = path.scope.getBinding(callee.name);
  if (!binding || !t.isImportSpecifier(binding.path.node)) {
    return null;
  }

  const declaration = binding.path.parentPath;
  if (!declaration || !t.isImportDeclaration(declaration.node)) {
    return null;
  }

  if (declaration.node.source.value !== '@atlaskit/tokens') {
    return null;
  }

  return getNonAliasedImportName(binding.path.node) === 'token' ? binding : null;
}

function getNonAliasedImportName(node) {
  if (t.isIdentifier(node.imported)) {
    return node.imported.name;
  }
  return node.imported.value;
}

function isExempted(tokenName, exemptions) {
  return exemptions.some((prefix) => tokenName.startsWith(prefix));
}

function escapeForTemplateLiteral(value) {
  return value.replace(/\\|`|\${/g, '\\$&');
}

function getDefaultFallback(tokenName, theme) {
  if (shapeValues[tokenName]) {
    return shapeValues[tokenName];
  }
  if (spacingValues[tokenName]) {
    return spacingValues[tokenName];
  }
  if (typographyValues[tokenName]) {
    return typographyValues[tokenName];
  }
  const colorValues = theme === 'legacy-light' ? legacyLightValues : lightValues;
  return colorValues[tokenName];
}

function getThemeValues(theme) {
  return theme.reduce((formatted, rawToken) => {
    let value;
    if (typeof rawToken.value === 'string') {
      value = rawToken.value;
    } else if (typeof rawToken.value === 'number') {
      value = rawToken.value.toString();
    } else if (Array.isArray(rawToken.value)) {
      value = rawToken.value.reduce((prev, curr, index) => {
        let color = curr.color;
        if (color.length === 7 && curr.opacity) {
          let opacityAsHex = curr.opacity.toString(16);
          let shortenedHex = opacityAsHex.slice(2, 4);
          if (shortenedHex.length === 1) {
            shortenedHex += '0';
          }
          color += shortenedHex;
        }
        let fragment = `${curr.offset.x}px ${curr.offset.y}px ${curr.radius}px ${color}`;
        if (index === 0) {
          fragment += ', ';
        }
        return prev + fragment;
      }, '');
    } else {
      return formatted;
    }

    formatted[rawToken.cleanName] = value;
    return formatted;
  }, {});
}

async function collectFixtureInputs(dir) {
  const files = [];
  const entries = await fsp.readdir(dir, { withFileTypes: true });
  for (const entry of entries) {
    const entryPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await collectFixtureInputs(entryPath)));
    } else if (entry.isFile() && /^in\.(jsx|tsx)$/.test(entry.name)) {
      files.push(entryPath);
    }
  }
  return files;
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
