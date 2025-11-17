import { styled } from '@compiled/react';
import { cssMap, cx } from '@atlaskit/css';

const DescriptionWrapper = styled.div({
  marginTop: "var(--space-100, 4px)"
});

const TextWrapper = styled.div({
  margin: 0,
});

const styles = cssMap({
  bannerContainer: {
    marginTop: "var(--space-100, 4px)",
    marginBottom: "var(--space-100, 4px)",
  },
});

export const Component = ({ showBanner }) => (
  <DescriptionWrapper>
    <TextWrapper>One</TextWrapper>
    {showBanner ? <div xcss={cx(styles.bannerContainer)} /> : null}
  </DescriptionWrapper>
);
