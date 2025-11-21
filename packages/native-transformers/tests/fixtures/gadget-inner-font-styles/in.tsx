/** @jsx jsx */
import React from 'react';
import { css, jsx } from '@compiled/react';
import { token } from '@atlaskit/tokens';

export const Gadget = () => {
  const fullWidthStyles = css({
    width: '100%',
  });

  return <div css={[fullWidthStyles, fontStyles]} />;
};

const fontStyles = css({
  fontFamily: token('font.family.body'),
});
