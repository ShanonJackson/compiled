/** @jsx jsx */
import React, { type FC } from 'react';
import { jsx } from '@compiled/react';
import { css } from '@atlaskit/css';
import { token } from '@atlaskit/tokens';

type Props = {
  children?: React.ReactNode;
};

export const MainContainer: FC<Props> = ({ children, ...props }) => {
  return (
    <div css={styles.container} {...props}>
      {children}
    </div>
  );
};

const styles = css({
  display: 'flex',
  flexFlow: 'column nowrap',
  borderColor: token('color.border'),
  borderStyle: 'solid',
  borderRadius: token('radius.large'),
  overflow: 'hidden',
  position: 'relative',
});
