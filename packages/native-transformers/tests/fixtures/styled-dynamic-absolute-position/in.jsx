import { styled } from '@compiled/react';


const DotStart = styled.div({
  position: 'absolute',
  top: (props) => `${props.y}px`,
  left: (props) => `${props.x}px`,
  borderRadius: 'var(--ds-radius-full, 9999px)',
  width: '10px',
  height: '10px',
  transform: 'translate(-5px, -5px)',
  backgroundColor: 'var(--ds-background-accent-blue-subtler, #CFE1FD)',
});

const DotEnd = styled(DotStart)({
  backgroundColor: 'var(--ds-background-accent-red-subtler, #FFD5D2)',
});

export const Example = () => <DotEnd x={10} y={20} />;
