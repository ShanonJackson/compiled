;
import * as React from "react";
import { ax, ix } from "@compiled/react/runtime";
const base = {
    fontSize: '16px',
    color: 'purple',
    padding: '8px'
};
const overrides = {
    color: 'purple',
    padding: '10px',
    border: '1px solid black'
};
const conditional = {
    ...(true ? {
        color: 'teal'
    } : {}),
    backgroundColor: 'beige'
};
export const spreadExample = null;
