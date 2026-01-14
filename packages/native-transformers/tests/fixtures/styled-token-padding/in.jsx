import React from 'react';
import { styled } from '@compiled/react';


const Wrapper = styled.div({
	padding: `${'var(--ds-space-050, 4px)'} ${'var(--ds-space-150, 12px)'} ${'var(--ds-space-150, 12px)'} ${({ padded }) =>
		padded ? 'var(--ds-space-150, 12px)' : 'var(--ds-space-0, 0px)'}`,
});

export const Component = () => <Wrapper padded />;
