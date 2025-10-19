import * as React from 'react';
import { ax, ix, CC, CS } from "@compiled/react/runtime";
import { jsx, jsxs } from "react/jsx-runtime";
const _ = "@property --my-color{syntax:\"<color>\";inherits:var(--_ikpmwb);initial-value:#000}";
const styles = null;
export const Component = () => jsxs(CC, {
  children: [jsx(CS, {
    children: [_]
  }), jsx("div", {
    className: ax([]),
    style: {
      "--_ikpmwb": ix(false)
    },
    children: "Hello"
  })]
});
