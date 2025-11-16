import { styled } from '@compiled/react';
import { token } from '@atlaskit/tokens';

const ModalBodyWrapper = styled.div({
  display: 'flex',
  flexDirection: 'column',
  gap: `${token('space.200')}`,
});

const ProgressBarWrapper = styled.div({
  display: 'flex',
  gap: `${token('space.200')}`,
  alignItems: 'center',
  alignSelf: 'stretch',
});

export const Example = ({ isAlternate }: { isAlternate?: boolean }) => (
  <div>
    <ModalBodyWrapper data-testid={isAlternate ? 'alt' : 'default'}>
      Content
    </ModalBodyWrapper>
    <ProgressBarWrapper>
      <span>Hello</span>
    </ProgressBarWrapper>
  </div>
);
