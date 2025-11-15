import type { Plugin } from 'postcss';

/**
 * PostCSS plugin which will callback when traversing through each root declaration.
 */
export const extractStyleSheets = (opts?: { callback: (sheet: string) => void }): Plugin => {
  return {
    postcssPlugin: 'extract-style-sheets',
    OnceExit(root) {
      root.each((node) => {
        const sheet = node.toString();
        if (process.env.COMPILED_CLI_TRACE) {
          // Log each final sheet in the order they are emitted                                                                                                                                                                             
          console.log('[css] extract sheet:', sheet);
        }
        opts?.callback(node.toString());
      });
    },
  };
};

export const postcss = true;
