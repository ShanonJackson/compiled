import React from 'react';
import { styled } from '@compiled/react';


export const Component = (props) => {
  // eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled -- parity with production usage
  const Label = styled.h5({
    color: props.isDisabled ? 'var(--ds-text-disabled, #080F214A)' : 'var(--ds-text, #292A2E)',
  });

  return <Label>text</Label>;
};
