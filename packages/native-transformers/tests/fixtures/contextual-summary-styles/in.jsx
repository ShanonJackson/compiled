import { css, jsx } from '@compiled/react';
import { token } from '@atlaskit/tokens';

const media = {
  above: {
    xxs: '@media (min-width: 0rem)',
    sm: '@media (min-width: 48rem)',
  },
};

const styles = css({
  alignItems: 'center',
  display: 'flex',
  marginTop: token('space.400'),
  paddingLeft: token('space.100'),
  background: token('elevation.surface'),
  borderRadius: token('radius.small'),
  gap: token('space.100'),
  [media.above.xxs]: {
    flexDirection: 'column',
  },
  [media.above.sm]: {
    flexDirection: 'row',
  },
  span: {
    color: `${token('color.text')} !important`,
  },
});

export const Component = () => (
  <div css={styles}>
    <span>Hi</span>
  </div>
);
