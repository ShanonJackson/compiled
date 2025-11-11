import { ax, ix } from '@compiled/react/runtime';
import { forwardRef as forwardRef } from 'react';
import * as React from 'react';
const Container = forwardRef(({ as: C = 'div', style: __cmpls, ...__cmplp }, __cmplr) => {
  if (__cmplp.innerRef) {
    throw new Error("Please use 'ref' instead of 'innerRef'.");
  }
  return (
    <C
      {...__cmplp}
      style={__cmpls}
      ref={__cmplr}
      className={ax(['_syaz5scu _30l313q2', __cmplp.className])}
    />
  );
});
if (process.env.NODE_ENV !== 'production') {
  Container.displayName = 'Container';
}
export const Example = () => <Container>hello world</Container>;
