import { css, jsx } from '@compiled/react';

const baseRow = css({
  height: '32px',
  marginTop: 'var(--ds-space-100, 8px)',
});

const wideRow = css({
  width: '100%',
});

const narrowRow = css({
  width: '20%',
});

export const Component = ({ narrow }) => (
  <div css={[baseRow, narrow ? narrowRow : wideRow]}>skeleton</div>
);
