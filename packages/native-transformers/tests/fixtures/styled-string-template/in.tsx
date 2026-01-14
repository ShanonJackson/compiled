import { styled } from '@compiled/react';
import { gridSize } from './grid-size';

const nameStyles = `
  margin-right: ${gridSize * 3}px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
`;

const NameLink = styled.a(nameStyles);

export const Component = () => <NameLink>text</NameLink>;
