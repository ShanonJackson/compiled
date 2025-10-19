import { forwardRef } from 'react';
import * as React from 'react';
import { ax, ix, CC, CS } from "@compiled/react/runtime";
import { jsx, jsxs } from "react/jsx-runtime";
const _ = "._syaz5scu{color:red}";
export const Styled = forwardRef(({
  as: C = "div",
  style: __cmpls,
  ...__cmplp
}, __cmplr) => {
  return jsxs(CC, {
    children: [jsx(CS, {
      children: [_]
    }), jsx(C, {
      ...__cmplp,
      style: __cmpls,
      ref: __cmplr,
      className: ax(["_syaz5scu", __cmplp.className])
    })]
  });
});
if (process.env.NODE_ENV !== 'production') {
  Styled.displayName = 'Styled';
}
