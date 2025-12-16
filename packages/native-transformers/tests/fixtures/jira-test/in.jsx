/**
 * @jsxRuntime classic
 * @jsx jsx
 */
import { jsx, cssMap } from '@compiled/react';

const styles = cssMap({
  padding: {
    padding: 'var(--_xn0y17) /*pad*/',
  },
  rows: {
    gridTemplateRows: '/*rows-a*/ var(--_1d7lvij)',
  },
  textColor: {
    textDecorationColor: '/*color-a*/ currentColor',
  },
  textColorImportant: {
    textDecorationColor: 'currentColor /*color-b*/ !important',
  },
  columnsA: {
    gridTemplateColumns: 'var(--_uu7jbp) /*col-a*/',
  },
  columnsB: {
    gridTemplateColumns: '/*col-b*/ var(--_183xcfk)',
  },
  paddingLeft: {
    paddingLeft: 28,
  },
  rowsB: {
    gridTemplateRows: 'var(--_rrxz8m) /*rows-b*/',
  },
  textInitial: {
    textDecorationColor: 'initial /*color-c*/',
  },
  textInitialImportant: {
    textDecorationColor: 'initial /*color-d*/ !important',
  },
  paddingRight: {
    paddingRight: 4,
  },
  columnsC: {
    gridTemplateColumns: '/*col-c*/ var(--_1w2vits)',
  },
  columnsD: {
    gridTemplateColumns: 'var(--_1369mr3) /*col-d*/',
  },
});

export const Fixture = () => (
  <div
    css={[
      styles.padding,
      styles.rows,
      styles.textColor,
      styles.textColorImportant,
      styles.columnsA,
      styles.columnsB,
      styles.paddingLeft,
      styles.rowsB,
      styles.textInitial,
      styles.textInitialImportant,
      styles.paddingRight,
      styles.columnsC,
      styles.columnsD,
    ]}
  >
    content
  </div>
);
