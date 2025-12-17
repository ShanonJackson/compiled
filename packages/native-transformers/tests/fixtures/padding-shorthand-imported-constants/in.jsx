import { css } from '@compiled/react';
import { ACTIONS_CELL_PADDING, TABLE_BUTTON_OUTLINE } from './constants';

const styles = css({
  display: 'flex',
  justifyContent: 'flex-end',
  padding: `${ACTIONS_CELL_PADDING} ${TABLE_BUTTON_OUTLINE} 0 0`,
});

export const Component = () => <div css={styles} />;
