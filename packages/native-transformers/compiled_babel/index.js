'use strict';

const path = require('path');

const { loadBinding } = require('@node-rs/helper');

let binding;

function getBinding() {
  if (!binding) {
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
