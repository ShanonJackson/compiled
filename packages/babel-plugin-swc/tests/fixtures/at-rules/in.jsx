import { css } from '@compiled/react';

export const className = css({
  '@property --accent-color': {
    syntax: '"<color>"',
    inherits: false,
    'initial-value': 'rebeccapurple',
  },
  '@media (max-width: 1024px)': {
    fontSize: 16,
  },
});
