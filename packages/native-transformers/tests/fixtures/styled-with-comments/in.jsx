/** @jsx jsx */
import React from 'react';
import { css, jsx, styled } from '@compiled/react';

const badgeStyles = css({
  display: 'inline-flex',
  alignItems: 'center',
  fontSize: 10,
  fontWeight: 600,
  textTransform: 'uppercase',
  color: '#1D7AFC',
  letterSpacing: 0.5,
  ':before': {
    content: '"•"',
    marginRight: 4,
  },
});

const PopUpListItem = styled.div({
  borderRadius: 'var(--ds-radius-small, 4px)',
  backgroundColor: 'var(--ds-background-neutral-bold, #44546f)',
  color: 'var(--ds-text-inverse, #fff)',
  padding: '8px 12px',
  maxWidth: 320,
});

const PopUpList = styled.div({
  display: 'flex',
  gap: 8,
  marginTop: 8,
});

export const Component = ({ services = [] }) => (
  <div>
    <span css={badgeStyles}>{services.length} services</span>
    <PopUpList>
      <PopUpListItem>{services.join(', ') || 'No services'}</PopUpListItem>
    </PopUpList>
  </div>
);
