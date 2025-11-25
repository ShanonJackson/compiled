import { css } from '@compiled/react';

const wrapper = css({
  display: 'flex',
  // descendant selector with attribute
  "& [data-testid='child']": {
    padding: 0,
  },
  "& [data-testid='header']": {
    margin: 0,
  },
});

export const Example = () => (
  <div css={wrapper}>
    <div data-testid="header">Header</div>
    <div data-testid="child">Body</div>
  </div>
);
