import React from 'react';
import { styled } from '@compiled/react';
import { token } from '@atlaskit/tokens';

const extraTopOffset = -1;

const StickyWrapper = styled.div(
  {
    '@supports (position: sticky) or (position: -webkit-sticky)': {
      position: 'sticky',
      background: (props) =>
        props.isEmbedMode ? token('elevation.surface.overlay') : token('elevation.surface'),
      zIndex: ({ zIndex }) => zIndex,
      marginLeft: token('space.negative.100'),
      top: (props) => `${props.topOffset + extraTopOffset}px`,
      boxShadow: (props) =>
        props.showKeyline ? `0 ${token('space.025')} ${token('color.border')}` : undefined,
      paddingBottom: ({ showKeyline, applyVisualRefreshChanges }) =>
        showKeyline && applyVisualRefreshChanges ? token('space.100') : undefined,

      paddingTop: ({ showKeyline, applyVisualRefreshChanges, showPaddingTopOnlyOnKeyline }) => {
        if (showPaddingTopOnlyOnKeyline) {
          return showKeyline && applyVisualRefreshChanges
            ? token('space.100')
            : `${-extraTopOffset}px`;
        }
        return applyVisualRefreshChanges ? token('space.100') : `${-extraTopOffset}px`;
      },
      paddingRight: ({ isTabLayout }) => (isTabLayout ? token('space.300') : undefined),
      paddingLeft: ({ isTabLayout, isSidebarPanelOpened }) =>
        // eslint-disable-next-line no-nested-ternary
        isSidebarPanelOpened ? 'unset' : isTabLayout ? token('space.300') : token('space.100'),
    },
  },
  ({ flexStyle, isWideLayout }) =>
    flexStyle && {
      display: 'flex',
      justifyContent: 'space-between',
      maxWidth: isWideLayout ? '1920px' : 'inherit',
      width: isWideLayout ? '100%' : 'inherit',
      justifySelf: isWideLayout ? 'center' : 'inherit',
    }
);

const Fixture = () => (
  <StickyWrapper
    topOffset={0}
    showKeyline
    isEmbedMode
    zIndex={10}
    applyVisualRefreshChanges
    flexStyle
    isWideLayout
    isTabLayout
    showPaddingTopOnlyOnKeyline
    isSidebarPanelOpened={false}
  >
    content
  </StickyWrapper>
);

export default Fixture;
