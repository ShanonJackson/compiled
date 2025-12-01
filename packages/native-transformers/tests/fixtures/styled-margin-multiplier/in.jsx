import { styled } from '@compiled/react';

const marginMultiplier = (hasScrolled, isShadowVisible) => {
	if (isShadowVisible) return 0;
	if (hasScrolled) return -1;
	return 1;
};

const getMargin = (hasScrolled, isShadowVisible) =>
	8 * marginMultiplier(hasScrolled, isShadowVisible);

const Wrapper = styled.div({
	marginLeft: ({ hasScrolled, leftShadowVisible }) =>
		`${getMargin(hasScrolled, leftShadowVisible)}px`,
	marginRight: ({ hasScrolled, rightShadowVisible }) =>
		`${getMargin(hasScrolled, rightShadowVisible)}px`,
	display: 'flex',
});

export const Component = (props) => <Wrapper {...props}>Content</Wrapper>;
