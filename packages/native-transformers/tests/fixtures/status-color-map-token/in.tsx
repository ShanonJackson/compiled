import { styled } from '@compiled/react';
import { token } from '@atlaskit/tokens';

const STATUS = {
  TODO: 'TODO',
  IN_PROGRESS: 'IN_PROGRESS',
  DONE: 'DONE',
  UNKNOWN: 'UNKNOWN',
} as const;

const STATUS_COLOR_MAP = {
  [STATUS.TODO]: token('color.icon.accent.gray'),
  [STATUS.IN_PROGRESS]: token('color.icon.accent.blue'),
  [STATUS.DONE]: token('color.icon.accent.green'),
  [STATUS.UNKNOWN]: 'unset',
};

export const StatusBar = styled.div<{ status: keyof typeof STATUS }>({
  backgroundColor: ({ status }) => STATUS_COLOR_MAP[status],
  top: 0,
  left: 0,
  right: 0,
  height: token('space.050'),
  borderRadius: `${token('radius.medium')} ${token('radius.medium')} 0 0`,
  position: 'absolute',
});
