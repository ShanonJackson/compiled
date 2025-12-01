import { styled } from '@compiled/react';

const Container = styled.span({
	width: '75%',
	'@media (min-aspect-ratio: 11 / 6)': {
		maxWidth: '50%',
	},
});

export const Component = () => <Container>preview</Container>;
