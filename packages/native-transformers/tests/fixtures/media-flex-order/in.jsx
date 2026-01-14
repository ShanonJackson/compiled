import { css } from '@compiled/react';

const card = css({
  display: 'none',
  '@media (min-width: 90rem)': {
    display: 'flex',
    flex: '1',
    justifyContent: 'center',
    alignItems: 'center',
    position: 'relative',
    overflow: 'hidden',
    minWidth: '654px',
    paddingTop: '16px',
    paddingBottom: '16px',
  },
});

export const Example = () => <div css={card} />;
