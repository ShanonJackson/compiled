import * as React from 'react';
import { ax, ix, CC, CS } from "@compiled/react/runtime";
import { palette } from './palette';
import { jsx, jsxs } from "react/jsx-runtime";
const _2 = "._irr3u67f:hover{background-color:#fff}";
const _ = "._syaz1ejv{color:rgba(10,20,30,.8)}";
const styles = null;
export const Component = () => jsxs(CC, {
  children: [jsx(CS, {
    children: [_, _2]
  }), jsx("div", {
    className: ax(["_syaz1ejv _irr3u67f"]),
    children: "imported twice"
  })]
});
