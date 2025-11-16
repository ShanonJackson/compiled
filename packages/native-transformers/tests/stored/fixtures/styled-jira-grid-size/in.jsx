import React from 'react';
import { styled } from '@compiled/react';
import { gridSize } from './jira-common-styles';

const Container = styled.div({
  width: `${gridSize * 2}px`,
  minWidth: `${gridSize * 75}px`,
  maxWidth: `${gridSize * 150}px`,
});

export const Component = () => <Container />;
