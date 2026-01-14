/** @jsx jsx */
import { jsx } from '@compiled/react';
import * as styles from './styles.module.css';
import { styled } from '@compiled/react';

const gridAreas = () => {
  if (styles.primary && styles.tertiary && styles.secondary) {
    return `"${styles.primary} ${styles.tertiary} ${styles.secondary}"`;
  }
  return '"primary tertiary secondary"';
};

const Footer = styled.div({
  gridTemplateAreas: gridAreas(),
});

export const Component = () => <Footer />;
