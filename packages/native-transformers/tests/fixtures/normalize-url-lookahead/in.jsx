import { styled } from '@compiled/react';

const Box = styled.div({
  backgroundImage: 'url(//example.com/image.png)',
  maskImage: 'url(example.com/mask.svg)',
});

export const Component = () => <Box />;
