/**
 * @jsxRuntime classic
 * @jsx jsx
 */
import React from 'react';
import { cssMap, jsx } from '@compiled/react';

const styles = cssMap({
	root: {
		flexGrow: '0.050',
	},
});

export const Component = () => <div css={styles.root} />;

