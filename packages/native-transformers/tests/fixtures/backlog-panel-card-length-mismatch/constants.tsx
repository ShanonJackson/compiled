import { token } from '@atlaskit/tokens';

const COLOR_TEXT = token('color.text');
const COLOR_TEXT_INVERSE = token('color.text.inverse');

export const cardColor = {
	default: {
		textColor: COLOR_TEXT,
		textColorSubtle: token('color.text.subtle'),
		progressBar: {
			textColor: token('color.text.subtlest'),
		},
		mouseLeave: {
			backgroundColor: token('elevation.surface.raised'),
			textColor: COLOR_TEXT,
		},
		mouseOver: {
			backgroundColor: token('elevation.surface.raised.hovered'),
			textColor: COLOR_TEXT,
		},
		mouseDown: {
			backgroundColor: token('color.background.neutral.subtle.pressed'),
		},
	},
	active: {
		progressBar: {
			textColor: COLOR_TEXT_INVERSE,
		},
		mouseLeave: {
			backgroundColor: token('color.background.accent.blue.subtlest'),
			textColor: COLOR_TEXT,
		},
		mouseOver: {
			backgroundColor: token('color.background.accent.blue.subtler'),
			textColor: COLOR_TEXT,
		},
		mouseDown: {
			backgroundColor: token('color.background.selected.pressed'),
		},
	},
};
