import * as React from 'react';
import { ax, ix, CC, CS } from "@compiled/react/runtime";
import { jsx, jsxs } from "react/jsx-runtime";
const _3 = "._1q7w1isi span:before{content:attr(data-label)}";
const _2 = "._aetr1e8g:after{content:\"hello\"}";
const _ = "._1kt9b3bt:before{content:\"\"}";
const styles = null;
export const Component = () => jsxs(CC, {
  children: [jsx(CS, {
    children: [_, _2, _3]
  }), jsx("div", {
    className: ax(["_1kt9b3bt _aetr1e8g _1q7w1isi"]),
    children: jsx("span", {
      "data-label": "test"
    })
  })]
});
