import * as React from 'react';
import { ax, ix, CC, CS } from "@compiled/react/runtime";
import { jsx, jsxs } from "react/jsx-runtime";
const styles = {
  primary: "_syazso35",
  secondary: "_syaz1iu8"
};
export const Component = () => jsxs("div", {
  children: [jsx("span", {
    className: styles.primary(),
    children: "Primary"
  }), jsx("span", {
    className: styles.secondary(),
    children: "Secondary"
  })]
});
