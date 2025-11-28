import React from 'react';
import { cssMap } from '@atlaskit/css';
import { Box } from '@atlaskit/primitives/compiled';
import { token } from '@atlaskit/tokens';

export default function Component() {
  return <Box xcss={styles.cellWrapper}>Timeline cell</Box>;
}

const styles = cssMap({
  cellWrapper: {
    height: '100%',
    inset: token('space.0'),
    paddingRight: token('space.100'),
    paddingLeft: token('space.100'),
    position: 'relative',
    width: '100%',
    alignItems: 'center',
    display: 'flex',
    overflowX: 'hidden',
  },
});
