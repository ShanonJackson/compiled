import { css } from '@compiled/react';

const gridSize = 8;
const ACTIONS_CELL_PADDING = `${gridSize / 2}px`; // 4px
const TABLE_BUTTON_OUTLINE = `${gridSize / 4}px`; // 2px

const styles = css({
  display: 'flex',
  justifyContent: 'flex-end',
  padding: `${ACTIONS_CELL_PADDING} ${TABLE_BUTTON_OUTLINE} 0 0`,
});

export const Component = () => <div css={styles} />;
