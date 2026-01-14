'use strict';

const path = require('path');

let binding;

function getBinding() {
  if (!binding) {
    let loadBinding;
    try {
      ({ loadBinding } = require('@swc/core/node'));
    } catch (e1) {
      try {
        ({ loadBinding } = require('@swc/core/lib/node'));
      } catch (e2) {
        throw new Error(
          "Unable to load SWC binding loader '@swc/core/node'. For fixtures, prefer using the native CLI (fixtures_cli)."
        );
      }
    }

    try {
      binding = loadBinding(path.join(__dirname, 'native'), '@compiled', 'compiled_strip_runtime');
    } catch (error) {
      error.message =
        "Failed to load native binding for '@compiled/babel-plugin-strip-runtime'. Compile the Rust crate in packages/native-transformers/compiled_strip_runtime." +
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
