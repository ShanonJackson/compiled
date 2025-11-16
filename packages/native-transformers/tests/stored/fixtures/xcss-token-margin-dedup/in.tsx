import { styled } from '@compiled/react';
import { cssMap, cx } from '@atlaskit/css';
import { token } from '@atlaskit/tokens';

const DescriptionWrapper = styled.div({
  marginTop: token('space.100'),
});

const TextWrapper = styled.div({
  margin: 0,
});

const styles = cssMap({
  bannerContainer: {
    marginTop: token('space.100'),
    marginBottom: token('space.100'),
  },
});

export const Component = ({ showBanner }: { showBanner?: boolean }) => (
  <DescriptionWrapper>
    <TextWrapper>One</TextWrapper>
    {showBanner ? <div xcss={cx(styles.bannerContainer)} /> : null}
  </DescriptionWrapper>
);
