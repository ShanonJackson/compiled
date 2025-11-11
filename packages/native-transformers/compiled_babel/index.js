'use strict';

const path = require('path');

let binding;

function getBinding() {
  if (!binding) {
    let loadBinding;
    try {
      // Preferred entrypoint when available
      ({ loadBinding } = require('@swc/core/node'));
    } catch (e1) {
      try {
        // Older builds exposed a lib/node file
        ({ loadBinding } = require('@swc/core/lib/node'));
      } catch (e2) {
        throw new Error(
          "Unable to load SWC binding loader '@swc/core/node'. For fixtures, prefer using the native CLI (fixtures_cli)."
        );
      }
    }

    try {
      binding = loadBinding(path.join(__dirname, 'native'), '@compiled', 'compiled_babel');
    } catch (error) {
      error.message =
        "Failed to load native binding for '@compiled/babel-plugin'. Compile the Rust crate in packages/native-transformers/compiled_babel." +
        `\nOriginal error: ${error.message}`;
      throw error;
    }
  }
  return binding;
}

function unique(list) {
  const result = [];

  for (const value of list) {
    if (!result.includes(value)) {
      result.push(value);
    }
  }

  return result;
}

exports.transform = function transform(program, config) {
  const normalizedConfig = config || {};
  const { options: rawOptions, ...restConfig } = normalizedConfig;
  const { onIncludedFiles, ...options } = rawOptions || {};

  const result = getBinding().transform(program, {
    ...restConfig,
    options,
  });

  if (typeof onIncludedFiles === 'function') {
    const included = result?.metadata?.includedFiles || [];

    if (included.length > 0) {
      onIncludedFiles(unique(included));
    }
  }

  return result;
};

exports.load = getBinding;
