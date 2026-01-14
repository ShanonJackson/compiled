import { css } from '@compiled/react';
import { xcss } from '@atlaskit/primitives';
import { token } from '@atlaskit/tokens';

const legacyStyles = xcss({
  display: 'inline-flex',
  columnGap: 'space.050',
  maxWidth: '100%',
  alignItems: 'baseline',
  justifyContent: 'center',
});

const refreshedStyles = css({
  width: 'auto',
  maxWidth: '100%',
  alignItems: 'baseline',
  justifyContent: 'center',
  columnGap: token('space.050'),
  borderRadius: token('radius.small'),
  borderWidth: token('border.width'),
  borderStyle: 'solid',
  borderColor: 'transparent',
  maxHeight: '32px',
  font: token('font.body'),
  fontWeight: token('font.weight.medium'),
  paddingBlock: token('space.075'),
  paddingInline: token('space.150'),
  cursor: 'pointer',
  '&:focus, &:active, &:focus-visible': {
    outline: `${token('border.width.focused')} auto ${token('color.border.focused')}`,
    outlineOffset: token('space.negative.025'),
  },
});

export const Example = () => (
  <>
    <div xcss={legacyStyles}>legacy</div>
    <div css={refreshedStyles}>refresh</div>
  </>
);
