import { ax, ix, CC, CS } from "@compiled/react/runtime";
import { jsx, jsxs } from "react/jsx-runtime";
import * as React from "react";
import { forwardRef } from "react";
const _ = "._1ylx1my7{color:teal}";
const _1 = "._1uim1b2y{&:hover {\n    color: black}";
const StyledDiv = forwardRef(({ as: C = "div", style: __cmpls, ...__cmplp }, __cmplr)=>{
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
                    "_1ylx1my7",
                    "_1uim1b2y",
                    __cmplp.className
                ])
            })
        ]
    });
});
export const Component = ()=><StyledDiv>Hover me</StyledDiv>;
if (!process.env.NODE_ENV === "production") {
    StyledDiv.displayName = "StyledDiv";
}
