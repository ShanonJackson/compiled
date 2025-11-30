import React from 'react';
import { styled } from '@compiled/react';
import { token } from '@atlaskit/tokens';

const MENU_PLACEHOLDER_ID =
  'software-backlog.card-list.card.card-contents.context-menu.menu_placeholder';

const cardFocusStyles = {
  content: '',
  display: 'block',
  position: 'absolute',
  zIndex: 1,
  left: 0,
  right: 0,
  top: 0,
  bottom: 0,
  boxShadow: `0 0 0 2px ${token('color.border.focused')}`,
  pointerEvents: 'none',
};

const Container = styled.div({
  position: 'absolute',
  top: 0,
  bottom: 0,
  left: 0,
  right: 0,
  '&:focus': {
    outline: 'none',
    [`& ~ [data-component-selector="${MENU_PLACEHOLDER_ID}"]`]: {
      opacity: 1,
      visibility: 'visible',
    },
    '&::after': cardFocusStyles,
  },
  '@supports (-ms-ime-align: auto)': {
    '&:focus::after': {
      ...cardFocusStyles,
      boxShadow: `inset 0 0 0 2px ${token('color.border.focused')}`,
    },
  },
  '@media screen and (-ms-high-contrast: active), (-ms-high-contrast: none)': {
    '&:focus::after': {
      ...cardFocusStyles,
      boxShadow: `inset 0 0 0 2px ${token('color.border.focused')}`,
    },
  },
});

const Fixture = () => <Container data-testid="interaction-layer" />;

export default Fixture;
