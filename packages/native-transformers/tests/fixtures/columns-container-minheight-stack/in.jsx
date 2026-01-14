import { styled } from '@compiled/react';

// Mirror the sharedStyles export from board-kit.
const layout = { columnMargin: 6 };
const sharedStyles = {
  columnWithMarginWidth: 270 + layout.columnMargin * 2,
  zIndex: {
    boardBorders: 90,
  },
  columnFixedWidth: 270,
  columnMinWidth: 150,
  columnHeaderHeight: 48,
};

const fg = () => false;

export const Component = styled.div({
  display: ({ isFlexible }) => (isFlexible ? 'grid' : 'flex'),
  gridTemplateColumns: ({ isFlexible }) =>
    !isFlexible
      ? 'initial'
      : `repeat(auto-fit,minmax(${sharedStyles.columnMinWidth}px,1fr))`,
  gridAutoFlow: 'column',
  flex: '1 1 auto',
  minHeight: ({ isFlexible, isSwimlaneMode }) =>
    isFlexible && !isSwimlaneMode
      ? fg('avoid_board_scroll_container_style_changes')
        ? '100%'
        : 'calc(var(--board-scroll-element-height) * 1px - 8px)'
      : undefined,
  width: ({ shouldShowSampleProjectDataNudge }) =>
    shouldShowSampleProjectDataNudge ? 'max-content' : undefined,
});

export default Component;
