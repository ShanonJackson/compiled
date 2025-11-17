import { styled } from '@compiled/react';


const SkeletonRow = styled.div({
	backgroundImage: `linear-gradient(
    to right,
    ${'var(--ds-background-neutral, #0515240F)'} 10%,
    ${'var(--ds-background-neutral-subtle, #00000000)'} 30%,
    ${'var(--ds-background-neutral, #0515240F)'} 50%
  )`,
	backgroundRepeat: 'no-repeat',
	height: (props) => `${props.height}px`,
	width: (props) => `${props.width}px`,
	borderRadius: 'var(--ds-radius-small, 3px)',
});

export const Component = () => <SkeletonRow height={40} width={200} />;
