import React from 'react';
import { styled } from '@compiled/react';

// Regression: content expression with a regex literal should not crash keyframes hashing.
const LegendName = styled.span<{ name?: string }>({
  '&::after': {
    content: (props) => JSON.stringify((props.name || '').replace(/\\/g, '')),
  },
});

export const Component = () => <LegendName name="foo\\bar" />;
