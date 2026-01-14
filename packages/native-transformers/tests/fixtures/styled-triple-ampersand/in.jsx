import React from 'react';
import { styled } from '@compiled/react';

const BaseLink = (props) => <a {...props} />;

const LinkStyled = styled(BaseLink)({
	'&&&': {
		fontWeight: 'inherit',
	},
	textDecoration: 'underline',
});

export const Component = () => <LinkStyled href="#">Help link</LinkStyled>;
