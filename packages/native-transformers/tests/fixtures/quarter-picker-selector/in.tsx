import { styled } from '@compiled/react';

const QuarterPickerContainer = styled.div({
  '> *, > button': {
    flex: '0 0 50%',
    height: '105px',
    margin: '0 0 5px',
    '&:hover, &:disabled': {
      height: '105px',
    },
  },
  '> button > span': {
    alignSelf: 'center',
  },
});

const QuarterPicker = () => (
  <QuarterPickerContainer>
    <button>
      <span>Custom</span>
    </button>
    <div>Quarter</div>
  </QuarterPickerContainer>
);

export default QuarterPicker;
