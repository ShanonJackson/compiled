import { forwardRef } from 'react';
import * as React from 'react';
import { ax, ix, CC, CS } from "@compiled/react/runtime";
import { jsx, jsxs } from "react/jsx-runtime";
const _2 = "._30l3r3uz:hover{color:#000}";
const _ = "._syaz1my7{color:teal}";
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
      children: [_, _2]
    }), jsx(C, {
      ...__cmplp,
      style: __cmpls,
      ref: __cmplr,
      className: ax(["_syaz1my7 _30l3r3uz", __cmplp.className])
    })]
  });
});
if (process.env.NODE_ENV !== 'production') {
  StyledDiv.displayName = 'StyledDiv';
}
export const Component = () => jsx(StyledDiv, {
  children: "Hover me"
});
