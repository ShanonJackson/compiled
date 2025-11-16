import { styled } from '@compiled/react';

const gridSize = 8;

const StatusContainer = styled.div({
  margin: 0,
  gridTemplateColumns: ({ widthMultiplier }) => `repeat(auto-fit,${gridSize * widthMultiplier}px)`,
});

export const Component = ({ widthMultiplier }) => (
  <StatusContainer widthMultiplier={widthMultiplier}>
    <div />
  </StatusContainer>
);
