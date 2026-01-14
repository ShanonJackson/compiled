/** @jsx jsx */
import { css, jsx } from '@compiled/react';
import { token } from '@atlaskit/tokens';

const containerStylesOld = css({
  display: 'flex',
  flexDirection: 'column',
  position: 'relative',
  paddingTop: token('space.100'),
  paddingRight: token('space.200'),
  paddingBottom: token('space.100'),
  paddingLeft: token('space.200'),
  backgroundColor: token('elevation.surface.raised'),
  boxShadow: token('elevation.shadow.raised'),
});

const titleContainerStyles = css({
  display: 'flex',
  position: 'relative',
  flexDirection: 'row',
  alignItems: 'center',
  overflow: 'hidden',
});

const childContainerStyles = css({
  position: 'relative',
  paddingLeft: token('space.300'),
  marginTop: token('space.050'),
});

const DependencyCardFixture = ({ withParent = true }) => {
  const content = <div css={containerStylesOld}>content</div>;

  return withParent ? (
    <>
      <div css={titleContainerStyles}>title</div>
      <div css={childContainerStyles}>{content}</div>
    </>
  ) : (
    content
  );
};

export default DependencyCardFixture;
