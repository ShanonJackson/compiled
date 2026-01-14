import { styled } from '@compiled/react';

// Mirrors the Jira action-button Counter style that returns an arrow function
// which then returns a conditional string.
const Counter = styled.span<{ hasWatchers?: boolean }>({
  marginLeft: (props) => () => (props.hasWatchers ? 'var(--ds-space-025)' : 'var(--ds-space-0)'),
});

export default function Component({ hasWatchers }: { hasWatchers?: boolean }) {
  return <Counter hasWatchers={hasWatchers}>count</Counter>;
}
