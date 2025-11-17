/**
 * @jsxRuntime classic
 * @jsx jsx
 */
import { cssMap as cssMapUnbounded } from '@compiled/react';
import { cssMap, jsx } from '@atlaskit/css';


const stylesOld = cssMap({
	container: {
		display: 'inline-flex',
		borderRadius: 'var(--ds-radius-small, 4px)',
		blockSize: 'min-content',
		position: 'static',
		overflow: 'hidden',
		paddingInline: 'var(--ds-space-050, 4px)',
		boxSizing: 'border-box',
	},
	'text.bold.default': { color: 'var(--ds-text-inverse, #FFFFFF)' },
	'text.bold.inprogress': { color: 'var(--ds-text-inverse, #FFFFFF)' },
	'text.subtle.default': { color: 'var(--ds-text-subtle, #505258)' },
});

const stylesOldUnbounded = cssMapUnbounded({
	text: {
		fontFamily: 'var(--ds-font-family-body, ui-sans-serif, -apple-system, BlinkMacSystemFont, "Segoe UI", Ubuntu, "Helvetica Neue", sans-serif)',
		fontSize: '11px',
		fontStyle: 'normal',
		fontWeight: 'var(--ds-font-weight-bold, 700)',
		lineHeight: '16px',
		overflow: 'hidden',
		textOverflow: 'ellipsis',
		textTransform: 'uppercase',
		whiteSpace: 'nowrap',
	},
	customLetterspacing: {
		letterSpacing: 0.165,
	},
});

export const Lozenge = ({ children, appearance = 'default', isBold = false }) => (
	<div css={[stylesOld.container, stylesOldUnbounded.text, stylesOldUnbounded.customLetterspacing]}>
		<span css={stylesOld[`text.${isBold ? 'bold' : 'subtle'}.${appearance}`]}>
			{children}
		</span>
	</div>
);