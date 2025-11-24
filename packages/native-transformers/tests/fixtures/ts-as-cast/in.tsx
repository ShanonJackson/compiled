import React from 'react';
import { styled } from '@compiled/react';

type Props = { lineclamp: number | 'auto' };

const ClampedHighlight = styled.div({
	maxHeight: (props: Props) => `${(props.lineclamp as number) * 1.42857142857143}em`,
});

export const Example = (props: Props) => (
	<ClampedHighlight {...props}>example</ClampedHighlight>
);
