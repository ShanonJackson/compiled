/** @jsx jsx */
import { css, jsx } from '@compiled/react';
import { Inline, xcss } from '@atlaskit/primitives';

const ListWithPopup = ({
                         items,
                         ItemComponent,
                         maxLimit,
                         initialIsOpen,
                         isHoverPopoverEnabled,
                       }) => {
  return (
    <Inline
      space="space.100"
      alignBlock="center"
      shouldWrap={!isHoverPopoverEnabled}
      xcss={isHoverPopoverEnabled && hoverPopoverStyles}
    >
      <ItemComponent
        isHoverPopoverEnabled={isHoverPopoverEnabled}
        css={isHoverPopoverEnabled && hoverPopoverItemStyles}
      />
    </Inline>
  );
};

const hoverPopoverStyles = xcss({
  paddingRight: "var(--space-50, 50px)",
  width: '100%',
});

const hoverPopoverItemStyles = css({
  minWidth: "var(--space-0, 0px)",
});

export default ListWithPopup;
