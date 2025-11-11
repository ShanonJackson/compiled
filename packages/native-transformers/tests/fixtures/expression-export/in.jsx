import { css } from '@compiled/react';

const base = 10;
const offset = 4;
const colors = ['red', 'blue', 'seagreen'];

export const dynamicPadding = css`
  padding: ${(base + offset) * 2}px;
  color: ${colors[2]};
  &:hover {
    transform: translateX(${base / 2}px);
  }
`;
