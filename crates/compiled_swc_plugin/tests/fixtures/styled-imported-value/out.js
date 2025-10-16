import { colors } from './palette';
import { ax, ix, CC, CS } from "@compiled/react/runtime";
import { jsx, jsxs } from "react/jsx-runtime";
import * as React from "react";
import { forwardRef } from "react";
const _ = "._9ad0145m{color:rebeccapurple}";
const StyledDiv = forwardRef(({ as: C = "div", style: __cmpls, ...__cmplp }, __cmplr)=>{
    if (__cmplp.innerRef) {
        throw new Error("Please use 'ref' instead of 'innerRef'.");
    }
    return jsxs(CC, {
        children: [
            jsx(CS, {
                children: [
                    _
                ]
            }),
            jsx(C, {
                ...__cmplp,
                style: __cmpls,
                ref: __cmplr,
                className: ax([
                    "_9ad0145m",
                    __cmplp.className
                ])
            })
        ]
    });
});
export const Component = ()=><StyledDiv>Hi</StyledDiv>;
if (!process.env.NODE_ENV === "production") {
    StyledDiv.displayName = "StyledDiv";
}
