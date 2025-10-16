import { ClassNames } from '@compiled/react';
import { ax, ix, CC, CS } from "@compiled/react/runtime";
import { jsx, jsxs } from "react/jsx-runtime";
import * as React from "react";
import { forwardRef } from "react";
const _ = ":focus ._f1qk14u1{outline:none}";
const _1 = "._gq7jriy3{padding:8px}";
const fade = null;
const toneMap = {
    danger: "_9ad03wth",
    primary: "_9ad0j63n"
};
const baseStyles = null;
const Wrapper = forwardRef(({ as: C = "div", style: __cmpls, ...__cmplp }, __cmplr)=>{
    if (__cmplp.innerRef) {
        throw new Error("Please use 'ref' instead of 'innerRef'.");
    }
    return jsxs(CC, {
        children: [
            jsx(CS, {
                children: [
                    _,
                    _1
                ]
            }),
            jsx(C, {
                ...__cmplp,
                style: __cmpls,
                ref: __cmplr,
                className: ax([
                    "_f1qk14u1",
                    "_gq7jriy3",
                    __cmplp.className
                ])
            })
        ]
    });
});
export const Component = ()=>(<ClassNames>
    {({ css })=>(<Wrapper css={[
            baseStyles,
            toneMap.primary
        ]}>
        <span className={css({
            animation: `${fade} 2s linear`,
            ':after': {
                content: '"!"'
            }
        })}>
          combo
        </span>
      </Wrapper>)}
  </ClassNames>);
if (!process.env.NODE_ENV === "production") {
    Wrapper.displayName = "Wrapper";
}
