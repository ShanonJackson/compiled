import * as React from 'react';
import { token } from '@atlaskit/tokens';
import { ax, ix, CC, CS } from "@compiled/react/runtime";
import { jsx, jsxs } from "react/jsx-runtime";
import { forwardRef } from "react";
const _ = "._2x4gyor5 input{margin-top:var(--_1xq4qyj)}";
const _1 = "._12hvhfw9 input{margin-right:var(--_1m66l98)}";
const _2 = "._x5bdyor5 input{margin-bottom:var(--_1xq4qyj)}";
const _3 = "._1rgfhfw9 input{margin-left:var(--_1m66l98)}";
const _4 = "._1t2q30cd label{color:var(--_bp2er6)}";
const Wrapper = forwardRef(({ as: C = "div", style: __cmpls, ...__cmplp }, __cmplr)=>{
    if (__cmplp.innerRef) {
        throw new Error("Please use 'ref' instead of 'innerRef'.");
    }
    return jsxs(CC, {
        children: [
            jsx(CS, {
                children: [
                    _,
                    _1,
                    _2,
                    _3,
                    _4
                ]
            }),
            jsx(C, {
                ...__cmplp,
                style: {
                    ...__cmpls,
                    "--_1xq4qyj": ix(token('space.0')),
                    "--_1m66l98": ix(token('space.075')),
                    "--_bp2er6": ix(token('color.text.subtle'))
                },
                ref: __cmplr,
                className: ax([
                    "_2x4gyor5 _12hvhfw9 _x5bdyor5 _1rgfhfw9 _1t2q30cd",
                    __cmplp.className
                ])
            })
        ]
    });
});
export const Component = ()=>jsx(Wrapper, {
        children: jsx("label", {
            children: jsx("input", {
                type: "checkbox"
            })
        })
    });
if (process.env.NODE_ENV !== "production") {
    Wrapper.displayName = "Wrapper";
}
