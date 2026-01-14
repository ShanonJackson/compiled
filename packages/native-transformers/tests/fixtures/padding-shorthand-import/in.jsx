import { css } from '@compiled/react';
import { gridSize } from './grid-size';

const ACTIONS_CELL_PADDING = `${gridSize / 2}px`; // dynamic via import
const styles = css({
    display: 'flex',
    justifyContent: 'flex-end',
    padding: `${ACTIONS_CELL_PADDING} ${TABLE_BUTTON_OUTLINE} 0 0`,
});

export const Component = () => <div css={styles} />;
