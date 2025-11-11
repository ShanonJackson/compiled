import { styled } from '@compiled/react';

const Container = styled.div`
  color: red;
  &:hover {
    color: blue;
  }
`;

export const Example = () => <Container>hello world</Container>;
