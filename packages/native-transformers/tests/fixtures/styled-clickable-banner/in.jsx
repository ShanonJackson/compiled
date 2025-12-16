import { styled } from '@compiled/react';

const containerBackgrounds = {
	information: {
		default: 'info-default',
		hovered: 'info-hovered',
		active: 'info-active',
	},
	warning: {
		default: 'warn-default',
		hovered: 'warn-hovered',
		active: 'warn-active',
	},
};

const Container = styled.div({
	cursor: ({ hasAction }) => hasAction && 'pointer',
	background: ({ type }) => containerBackgrounds[type].default,
	paddingInline: ({ spacing }) => spacing,
	paddingBlock: '8px',
	'&:hover, &:focus': {
		background: ({ type }) => containerBackgrounds[type].hovered,
		'[data-component-selector="shortcut-icon-lQi8"]': {
			color: 'selected',
		},
	},
	'&:active': {
		background: ({ type }) => containerBackgrounds[type].active,
		'[data-component-selector="shortcut-icon-lQi8"]': {
			color: 'selected',
		},
	},
});

export const Component = ({ hasAction = true, type = 'information', spacing = '16px' }) => (
	<Container hasAction={hasAction} type={type} spacing={spacing}>
		Content
		<span data-component-selector="shortcut-icon-lQi8">icon</span>
	</Container>
);
