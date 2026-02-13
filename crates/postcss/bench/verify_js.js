#!/usr/bin/env node
// JS PostCSS 8.4.31 correctness harness — runs the first 3 plugins from transform.ts.
//
// Usage: node verify_js.js <input.css> <output.css>
//
// Applies discardDuplicates → discardEmptyRules → parentOrphanedPseudos
// (in that order, matching transform.ts) and writes the result.

const fs = require('fs');
const postcss = require('postcss');
const selectorParser = require('postcss-selector-parser');

// ── Plugin 1: discardDuplicates ─────────────────────────────────────────────
const discardDuplicates = () => ({
  postcssPlugin: 'discard-duplicates',
  Once(root) {
    const decls = {};
    root.each((node) => {
      if (node.type === 'decl') {
        decls[node.prop] = decls[node.prop] || [];
        decls[node.prop].push(node);
      }
    });
    for (const key in decls) {
      const found = decls[key];
      for (let i = 0; i < found.length - 1; i++) {
        found[i].remove();
      }
    }
  },
});
discardDuplicates.postcss = true;

// ── Plugin 2: discardEmptyRules ─────────────────────────────────────────────
const isValueEmpty = (value) =>
  value === 'undefined' || value === 'null' || value.trim() === '';

const discardEmptyRules = () => ({
  postcssPlugin: 'discard-empty-rules',
  Declaration(node) {
    if (isValueEmpty(node.value)) {
      const { parent } = node;
      node.remove();
      if (parent?.type === 'rule' && parent.nodes.length === 0) {
        parent.remove();
      }
    }
  },
});
discardEmptyRules.postcss = true;

// ── Plugin 3: parentOrphanedPseudos ─────────────────────────────────────────
const prependNestingTypeToSelector = (selector) => {
  const { parent } = selector;
  if (parent) {
    const nesting = selectorParser.nesting();
    parent.insertBefore(selector, nesting);
  }
};

const parentOrphanedPseudos = () => ({
  postcssPlugin: 'parent-orphened-pseudos',
  Once(root) {
    root.walkRules((rule) => {
      const { selectors } = rule;
      rule.selectors = selectors.map((selector) => {
        if (!selector.startsWith(':')) {
          return selector;
        }
        const parser = selectorParser((root) => {
          root.walkPseudos((pseudoSelector) => {
            prependNestingTypeToSelector(pseudoSelector);
          });
        }).astSync(selector, { lossless: false });
        return parser.toString();
      });
    });
  },
});
parentOrphanedPseudos.postcss = true;

// ── Main ────────────────────────────────────────────────────────────────────
const inputFile = process.argv[2];
const outputFile = process.argv[3];

if (!inputFile || !outputFile) {
  console.error('Usage: node verify_js.js <input.css> <output.css>');
  process.exit(1);
}

const css = fs.readFileSync(inputFile, 'utf-8');

const result = postcss([
  discardDuplicates(),
  discardEmptyRules(),
  parentOrphanedPseudos(),
]).process(css, { from: undefined });

fs.writeFileSync(outputFile, result.css);
