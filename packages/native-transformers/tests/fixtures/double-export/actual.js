import { css } from '@compiled/react';

const sharedClass = css({
  fontSize: '14px',
  color: 'rebeccapurple',
  backgroundColor: 'lavender',
});

const hoverHighlight = css`
  font-weight: bold;
  &:hover {
    color: hotpink;
  }
`;

export const first = sharedClass;
export const second = sharedClass;
export { sharedClass as default };

export const Example = () => (
  <div>
    <span css={sharedClass}>object styles</span>
    <span css={hoverHighlight}>template styles</span>
  </div>
);
