import { styled } from '@compiled/react';


const layoutStyles = {
	alignItems: 'center',
	display: 'flex',
	gap: `${'var(--ds-space-100, 8px)'}`,
};

const checkboxStyles = {
	...layoutStyles,
	borderBottom: `${'var(--ds-space-025, 2px)'} solid ${'var(--ds-border, #0B120E24)'}`,
	height: `${'var(--ds-space-500, 40px)'}`,
	width: '100%',
};

const getBackgroundColor = (checked, disabled) => {
	if (disabled) {
		return 'var(--ds-background-accent-gray-subtlest, #F0F1F2)';
	}

	return checked ? 'var(--ds-background-accent-blue-subtlest, #E9F2FE)' : 'transparent';
};

const ProjectCheckbox = styled.div({
	...checkboxStyles,
	backgroundColor: ({ checked, disabled }) =>
		getBackgroundColor(checked, disabled),
	input: {
		marginTop: 'var(--ds-space-0, 0px)',
		marginRight: 'var(--ds-space-075, 6px)',
		marginBottom: 'var(--ds-space-0, 0px)',
		marginLeft: 'var(--ds-space-075, 6px)',
	},
	label: {
		...layoutStyles,
	},
	p: {
		marginTop: 'var(--ds-space-0, 0px)',
		marginRight: 'var(--ds-space-0, 0px)',
		marginBottom: 'var(--ds-space-0, 0px)',
		marginLeft: 'var(--ds-space-0, 0px)',
	},
});

export const Component = () => (
	<ProjectCheckbox checked disabled={false}>
		<label>
			<input type="checkbox" />
			<p>Description</p>
		</label>
	</ProjectCheckbox>
);
