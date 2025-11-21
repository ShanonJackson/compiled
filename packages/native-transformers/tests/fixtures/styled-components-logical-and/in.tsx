import React from 'react';
import styled from 'styled-components';
import { token } from '@atlaskit/tokens';

// Styled-components usage with logical && interpolations should not panic in the Compiled plugin.
const DropdownTitle = styled.p<{ numItems?: number; enabled?: boolean }>`
  font: ${token('font.body.small')};
  ${({ enabled }) =>
    !enabled && `font-weight: ${token('font.weight.semibold')};`}
  ${({ numItems, enabled }) =>
    !enabled && `color: ${numItems ? token('color.text') : token('color.text.subtlest')};`}
`;

export const Component = () => <DropdownTitle numItems={1} />;
