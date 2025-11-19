import React from 'react';
import { styled } from '@compiled/react';


const Wrapper = styled.div({
	'> input::placeholder': {
		color: 'var(--ds-text, #292A2E)',
		fontWeight: 'var(--ds-font-weight-medium, 500)',
	},
});

export const Component = () => (
	<Wrapper>
		<input placeholder="Example" />
	</Wrapper>
);
