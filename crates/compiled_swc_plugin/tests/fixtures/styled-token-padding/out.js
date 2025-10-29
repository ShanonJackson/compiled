import { ax, ix, CC, CS } from "@compiled/react/runtime";
import * as _React from "react";
import React from 'react';
import { forwardRef } from "react";
import { token } from '@atlaskit/tokens';
import { jsx, jsxs } from "react/jsx-runtime";
const _ = "._1yt4mtij{padding:var(--ds-space-050,4px) var(--ds-space-150,9pt) var(--ds-space-150,9pt) var(--_udlqo0)}";
const Wrapper = forwardRef(({ as: C = "div", style: __cmpls, ...__cmplp }, __cmplr)=>{
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
                style: {
                    ...__cmpls,
                    "--_udlqo0": ix(__cmplp.padded ? token('space.150') : token('space.0'))
                },
                ref: __cmplr,
                className: ax([
                    "_1yt4mtij",
                    __cmplp.className
                ])
            })
        ]
    });
});
export const Component = ()=>jsx(Wrapper, {
        padded: true
    });
if (process.env.NODE_ENV !== "production") {
    Wrapper.displayName = "Wrapper";
}
