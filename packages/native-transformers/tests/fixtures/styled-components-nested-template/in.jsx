import { styled } from '@compiled/react';

const Styled = styled.div`
  margin: ${({ size }) => `${`${size}px`}`};
`;

export const Component = () => <Styled size={10} />;
