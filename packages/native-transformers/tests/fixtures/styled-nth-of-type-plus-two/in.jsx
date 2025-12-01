import { styled } from '@compiled/react';

// Nested pseudo selector using "n+2" form that currently hashes differently between Babel and SWC.
export const Body = styled.div({
  p: {
    '&:nth-of-type(n+2)': {
      marginTop: '8px',
    },
  },
});
