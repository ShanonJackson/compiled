import React from 'react';
import { styled } from '@compiled/react';
import { layers } from './source';

const Wrapper = styled.div({
	zIndex: layers.blanket,
	position: 'fixed',
});

export const Component = () => <Wrapper />;
