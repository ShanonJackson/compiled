import { css, styled } from '@compiled/react';

const Component = styled.div<{ flag?: boolean; other?: boolean }>`
  position: relative;
  ${({ flag }) => (flag ? css({ height: '10px' }) : css({}))}
  padding: 0 6px;
  ${({ other }) =>
    other ? css({ backgroundColor: 'pink', textDecoration: 'none' }) : css({})}
`;

export default Component;
