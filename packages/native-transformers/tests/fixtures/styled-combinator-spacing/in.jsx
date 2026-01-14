import * as React from 'react';
import { styled } from '@compiled/react';

const Copyable = styled.div({
  display: 'flex',
  '&>span': {
    maxWidth: 'calc(100% - 38px)',
    overflowX: 'hidden',
    overflowY: 'hidden',
    textOverflow: 'ellipsis',
  },
});

export const Component = () => (
  <Copyable>
    <span>details</span>
  </Copyable>
);
