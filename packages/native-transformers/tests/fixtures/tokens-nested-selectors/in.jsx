import { styled } from '@compiled/react';


const Wrapper = styled.div({
	input: {
		marginTop: 'var(--ds-space-0, 0px)',
		marginRight: 'var(--ds-space-075, 6px)',
		marginBottom: 'var(--ds-space-0, 0px)',
		marginLeft: 'var(--ds-space-075, 6px)',
	},
	label: {
		color: 'var(--ds-text-subtle, #505258)',
	},
});

export const Component = () => (
	<Wrapper>
		<label>
			<input type="checkbox" />
		</label>
	</Wrapper>
);
