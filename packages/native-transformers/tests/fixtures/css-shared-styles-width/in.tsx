import { styled } from '@compiled/react';
import { sharedStyles } from './sharedStyles';

const ColumnPlaceholder = styled.div({
  display: 'flex',
  justifyContent: 'flex-end',
  width: `${sharedStyles.columnFixedWidth}px`,
});

export const Component = () => <ColumnPlaceholder />;
