import { styled } from '@compiled/react';


const Box = styled.div({
	backgroundImage: `linear-gradient(
    to right,
    ${'var(--ds-background-neutral, #0515240F)'} 10%,
    ${'var(--ds-background-neutral-subtle, #00000000)'} 30%,
    ${'var(--ds-background-neutral, #0515240F)'} 50%
  )`,
});

export const Component = () => <Box />;
