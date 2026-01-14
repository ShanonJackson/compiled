/**
 * @jsxRuntime classic
 * @jsx jsx
 */
import React from 'react';
import { cssMap, cx, jsx } from '@compiled/react';


const anchorStyles = cssMap({
  root: {
    display: 'flex',
    alignItems: 'center',
    height: '32px',
    borderRadius: 'var(--ds-radius-small, 3px)',
  },
  newInteractionStates: {
    '&:hover': {
      backgroundColor: 'var(--ds-background-neutral-subtle-hovered, #0515240F)',
    },
    '&:active': {
      backgroundColor: `${'var(--ds-background-neutral-subtle-pressed, #0B120E24)'}!important`,
    },
  },
});

const logoContainerStyles = cssMap({
  root: {
    display: 'none',
    maxWidth: 320,
    boxSizing: 'content-box',
    paddingInline: 'var(--ds-space-100, 8px)',
    '@media (min-width: 64rem)': {
      '&&': {
        display: 'flex',
      },
    },
  },
});

const LogoRenderer = ({ logoOrIcon }) => {
  return <div>{logoOrIcon}</div>;
};

const Anchor = ({ children, xcss, ...props }) => {
  return <a {...props}>{children}</a>;
};

export const CustomLogo = ({ href, logo, icon, onClick, label }) => {
  return (
    <Anchor
      aria-label={label}
      href={href}
      xcss={cx(anchorStyles.root, anchorStyles.newInteractionStates)}
      onClick={onClick}>
      <div css={[logoContainerStyles.root]}>
        <LogoRenderer logoOrIcon={logo} />
      </div>
    </Anchor>
  );
};
