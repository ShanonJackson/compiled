import { css } from '@compiled/react';

const styles = css({
  display: 'flex',
  alignItems: 'center',
  '& > *': {
    marginLeft: 'var(--space-200, 4px)',
  },
  '& > *:first-child': {
    marginLeft: 0,
  },
  '& > *:last-child': {
    marginRight: 0,
  },
});

export const Component = () => (
  <div css={styles}>
    <span />
    <span />
    <span />
  </div>
);
