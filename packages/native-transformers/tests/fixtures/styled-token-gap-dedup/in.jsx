import { styled } from '@compiled/react';

const ModalBodyWrapper = styled.div({
  display: 'flex',
  flexDirection: 'column',
  gap: `${"var(--space-200, 4px)"}`,
});

const ProgressBarWrapper = styled.div({
  display: 'flex',
  gap: `${"var(--space-200, 4px)"}`,
  alignItems: 'center',
  alignSelf: 'stretch',
});

export const Example = ({ isAlternate }) => (
  <div>
    <ModalBodyWrapper data-testid={isAlternate ? 'alt' : 'default'}>
      Content
    </ModalBodyWrapper>
    <ProgressBarWrapper>
      <span>Hello</span>
    </ProgressBarWrapper>
  </div>
);
