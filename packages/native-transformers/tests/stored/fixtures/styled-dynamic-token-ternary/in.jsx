import React from 'react';
import { styled } from '@compiled/react';
import { Variables } from './tokens';

const Wrapper = styled.div({
  color: ({ withSidebar }) => (withSidebar ? undefined : Variables.COLOR_TEXT),
});

export const Component = ({ withSidebar }) => <Wrapper withSidebar={withSidebar} />;
