;
import * as React from "react";
import { ax, ix } from "@compiled/react/runtime";
const base = 10;
const offset = 4;
const colors = [
    'red',
    'blue',
    'seagreen'
];
export const dynamicPadding = null;
export const expressionObject = null;
export const ExpressionExample = ()=>(<div className={[
        dynamicPadding,
        expressionObject
    ].join(' ')}>
    expression output
  </div>);
