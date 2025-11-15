import type { ChildNode, Rule, Plugin, AtRule } from 'postcss';

import { sortPseudoSelectors } from '../utils/sort-pseudo-selectors';

import { parseMediaQuery } from './at-rules/parse-media-query';
import { sortAtRules } from './at-rules/sort-at-rules';
import type { AtRuleInfo } from './at-rules/types';
import { sortShorthandDeclarations } from './sort-shorthand-declarations';

const sortAtRulePseudoSelectors = (atRule: AtRule) => {
  const rules: Rule[] = [];

  atRule.each((childNode) => {
    switch (childNode.type) {
      case 'atrule':
        sortAtRulePseudoSelectors(childNode);
        break;

      case 'rule':
        rules.push(childNode.clone());
        childNode.remove();
        break;

      default:
        break;
    }
  });

  sortPseudoSelectors(rules);
  rules.forEach((rule) => {
    atRule.append(rule);
  });
};

/**
 * PostCSS plugin for sorting pseudo-selectors (inside and outside at-rules)
 * based on lvfha ordering, and the at-rules themselves as well.
 *
 * Only top level CSS rules will be sorted.
 *
 * Using Once due to the catchAll behaviour
 */
export const sortAtomicStyleSheet = (config: {
  sortAtRulesEnabled: boolean | undefined;
  sortShorthandEnabled: boolean | undefined;
}): Plugin => {
  const sortAtRulesEnabled = config.sortAtRulesEnabled ?? true;
  const sortShorthandEnabled = config.sortShorthandEnabled ?? true;

  return {
    postcssPlugin: 'sort-atomic-style-sheet',
    Once(root) {
      const catchAll: ChildNode[] = [];
      const rules: Rule[] = [];
      const atRules: AtRuleInfo[] = [];

      root.each((node) => {
        switch (node.type) {
          case 'rule': {
            if (node.first?.type === 'atrule') {
              atRules.push({
                parsed:
                  sortAtRulesEnabled && node.first.name === 'media'
                    ? parseMediaQuery(node.first.params)
                    : [],
                node,
                atRuleName: node.first.name,
                query: node.first.params,
              });
            } else {
              rules.push(node);
            }

            break;
          }

          case 'atrule': {
            atRules.push({
              parsed:
                sortAtRulesEnabled && node.name === 'media' ? parseMediaQuery(node.params) : [],
              node,
              atRuleName: node.name,
              query: node.params,
            });
            break;
          }

          default: {
            catchAll.push(node);
          }
        }
      });

      const firstDeclProp = (r: Rule): string =>
        r.nodes?.find((n): n is any => n.type === 'decl')?.prop ?? '<none>';

      console.log('[css] sort-atomic collected', {
        catchAll: catchAll.map((n) => n.type),
        rules: rules.map((r) => firstDeclProp(r)),
        atRules: atRules.map((a) => ({ name: a.atRuleName, query: a.query })),
      });

      if (sortShorthandEnabled) {
        sortShorthandDeclarations(catchAll);
        sortShorthandDeclarations(rules);
        sortShorthandDeclarations(atRules.map((atRule) => atRule.node));
        console.log('[css] sort-atomic after shorthand', {
          catchAll: catchAll.map((n) => n.type),
          rules: rules.map((r) => firstDeclProp(r)),
          atRules: atRules.map((a) => ({ name: a.atRuleName, query: a.query })),
        });
      }

      // Pseudo-selector and at-rule sorting takes priority over shorthand
      // property sorting.
      sortPseudoSelectors(rules);
      console.log('[css] sort-atomic after pseudo', {
        rules: rules.map((r) => firstDeclProp(r)),
        atRules: atRules.map((a) => ({ name: a.atRuleName, query: a.query })),
      });
      if (sortAtRulesEnabled) {
        atRules.sort(sortAtRules);
      }
      console.log('[css] sort-atomic after atRulesEnabled', {
        rules: rules.map((r) => firstDeclProp(r)),
        atRules: atRules.map((a) => ({ name: a.atRuleName, query: a.query })),
      });

      for (const atRule of atRules) {
        const node = atRule.node;
        if (node.type !== 'atrule') {
          continue;
        }
        sortAtRulePseudoSelectors(node);
      }
      console.log('[css] sort-atomic final', {
        order: [
          ...rules.map((r) => `rule(${firstDeclProp(r)}`),
          ...atRules.map((a) => `@${a.atRuleName} ${a.query}`),
      ]});
      root.nodes = [...catchAll, ...rules, ...atRules.map((atRule) => atRule.node)];
    },
  };
};

export const postcss = true;
