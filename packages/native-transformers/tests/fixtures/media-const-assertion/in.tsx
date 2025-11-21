import { css, jsx } from '@compiled/react';

const media = {
  above: {
    xs: '@media (min-width: 30rem)',
  } as const,
};

const styles = css({
  [media.above.xs]: {
    color: 'red',
  },
});

export const Component = () => <div css={styles}>hi</div>;
