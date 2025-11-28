/** @jsxRuntime classic */
/** @jsx jsx */
import { jsx, cssMap, cx } from '@atlaskit/css';
import { Box } from '@atlaskit/primitives/compiled';
import { token } from '@atlaskit/tokens';

const styles = cssMap({
	card: {
		borderRadius: token('radius.small'),
	},
	iconContainer: {
		borderRadius: token('radius.small', '3px'),
		textAlign: 'center',
	},
});

export const IconContainer = () => (
	<Box xcss={cx(styles.card)}>
		<Box as="span" xcss={cx(styles.iconContainer)}>
			icon
		</Box>
	</Box>
);
