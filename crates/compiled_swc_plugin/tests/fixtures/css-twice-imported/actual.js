import * as React from 'react';
import { palette } from './palette';
import { ax, ix, CC, CS } from "@compiled/react/runtime";
import { jsx, jsxs } from "react/jsx-runtime";
const _ = "._syaz1ejv{color:rgba(10,20,30,.8)}";
const _1 = "._irr3u67f:hover{background-color:#fff}";
const styles = null;
export const Component = ()=>(jsxs(CC, {
        children: [
            jsx(CS, {
                children: [
                    _,
                    _1
                ]
            }),
            jsx("div", {
                className: ax([
                    "_syaz1ejv _irr3u67f"
                ]),
                children: "imported twice"
            })
        ]
    }));
