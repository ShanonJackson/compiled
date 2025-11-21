/** @jsx jsx */
import React from 'react';
import { jsx } from '@compiled/react';
import { css } from '@atlaskit/css';
import { token } from '@atlaskit/tokens';

const styles = css({
  display: 'flex',
  flexFlow: 'column nowrap',
  borderColor: token('color.border'),
  borderStyle: 'solid',
  borderRadius: token('radius.large'),
});

export const Component = () => <div css={styles} />;
