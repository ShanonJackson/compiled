import { css } from '@compiled/react';

const sharedClass = css({
  fontSize: '14px',
  color: 'rebeccapurple',
  backgroundColor: 'lavender',
});

export const first = sharedClass;
export const second = sharedClass;
export { sharedClass as default };
