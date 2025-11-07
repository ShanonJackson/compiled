'use strict';

const path = require('path');
const { loadBinding } = require('@swc/core/node');

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

exports.transform = function transform(program, options) {
  return getBinding().transform(program, options);
};

exports.load = getBinding;
