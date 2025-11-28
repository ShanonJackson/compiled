import React from 'react';

import { cssMap } from '@atlaskit/css';
import { Pressable, Text } from '@atlaskit/primitives/compiled';
import { token } from '@atlaskit/tokens';

const styles = cssMap({
	trigger: {
		backgroundColor: 'transparent',
		width: 'fit-content',
		paddingTop: token('space.0'),
		paddingRight: token('space.0'),
		paddingBottom: token('space.0'),
		paddingLeft: token('space.0'),
		textDecoration: 'underline',
		borderRadius: token('radius.small'),
		'&:hover': {
			textDecoration: 'auto',
		},
	},
});

export const Example = () => (
	<Pressable xcss={styles.trigger}>
		<Text size="large" weight="bold" color="color.text">
			Trigger content
		</Text>
	</Pressable>
);
