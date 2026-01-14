import React from 'react';
import { styled } from '@compiled/react';


const Wrapper = styled.div({
	border: ({ isSummaryView }) =>
		isSummaryView ? 'none' : `1px solid ${'var(--ds-border, #0B120E24)'}`,
});

export const Component = () => <Wrapper isSummaryView={false} />;
