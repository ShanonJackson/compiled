import { styled } from '@compiled/react';

const MAP = { small: 16, medium: 24 } as const;

const Avatar = styled.div<{ size: 'small' | 'medium' }>({
  height: `${({ size }) => MAP[size]}px`,
  width: `${({ size }) => MAP[size]}px`,
  borderRadius: '4px',
});

export default Avatar;
