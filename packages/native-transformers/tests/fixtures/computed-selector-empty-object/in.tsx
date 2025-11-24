import React from 'react';
import { styled } from '@compiled/react';

const ICON_CONTAINER_SELECTOR = 'styled-category-item';

type Props = { error: boolean };

const Wrapper = styled.div({
	[`${ICON_CONTAINER_SELECTOR}`]: (props: Props) => (props.error ? {} : { display: 'none' }),
});

export const Example = (props: Props) => <Wrapper {...props}>content</Wrapper>;
