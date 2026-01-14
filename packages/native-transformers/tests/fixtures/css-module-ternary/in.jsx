/** @jsx jsx */
import { jsx, styled } from '@compiled/react';
import * as styles from './styles.module.css';

const FooterPrimarySection = styled.div({
  gridArea: styles.primary ? styles.primary : 'primary',
});

export const Component = () => <FooterPrimarySection />;
