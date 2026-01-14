import { styled } from '@compiled/react';

const Wrapper = styled.div({
  '& > [data-component="delete-question-button"]': {
    marginRight: '4px',
  },
  '&:not(:hover, :focus-within) > [data-component="delete-question-button"]': {
    opacity: 0,
  },
});

export const Example = () => (
  <Wrapper>
    <button data-component="delete-question-button">Delete</button>
  </Wrapper>
);
