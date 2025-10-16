import { forwardRef } from 'react';
import * as React from 'react';
import { ax, ix, CC, CS } from "@compiled/react/runtime";
import { colors } from './palette';
import { jsx, jsxs } from "react/jsx-runtime";
const _ = "._syaz15td{color:#639}";
const StyledDiv = forwardRef(({
  as: C = "div",
  style: __cmpls,
  ...__cmplp
}, __cmplr) => {
  if (__cmplp.innerRef) {
    throw new Error("Please use 'ref' instead of 'innerRef'.");
  }
  return jsxs(CC, {
    children: [jsx(CS, {
      children: [_]
    }), jsx(C, {
      ...__cmplp,
      style: __cmpls,
      ref: __cmplr,
      className: ax(["_syaz15td", __cmplp.className])
    })]
  });
});
if (process.env.NODE_ENV !== 'production') {
  StyledDiv.displayName = 'StyledDiv';
}
export const Component = () => jsx(StyledDiv, {
  children: "Hi"
});
