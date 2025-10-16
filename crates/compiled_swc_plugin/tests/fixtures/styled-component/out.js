import { ax, ix, CC, CS } from "@compiled/react/runtime";
import { jsx, jsxs } from "react/jsx-runtime";
import * as React from "react";
import { forwardRef } from "react";
const _ = "._9ad033t5{color:hotpink}";
const Base = ({ children })=><button>{children}</button>;
export const StyledButton = forwardRef(({ as: C = Base, style: __cmpls, ...__cmplp }, __cmplr)=>{
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
                    "_9ad033t5",
                    __cmplp.className
                ])
            })
        ]
    });
});
export const Component = ()=><StyledButton>Click me</StyledButton>;
if (!process.env.NODE_ENV === "production") {
    StyledButton.displayName = "StyledButton";
}
