/** @jsx jsx */
import { jsx, css } from '@compiled/react';

const hiddenTableContainerStyles = css({
  position: 'absolute',
  width: '100%',
  height: '100%',
  top: 0,
  left: 0,
  overflow: 'hidden',
  opacity: 0,
  pointerEvents: 'none',
  // eslint-disable-next-line @atlaskit/ui-styling-standard/no-nested-selectors -- fixture mirrors Jira usage
  '*': {
    pointerEvents: 'none',
  },
});

const Fixture = () => <div css={hiddenTableContainerStyles}>hidden</div>;

export default Fixture;
