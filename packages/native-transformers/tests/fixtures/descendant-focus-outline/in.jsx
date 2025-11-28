/**
 * @jsxRuntime classic
 * @jsx jsx
 */
import { cssMap, jsx } from '@compiled/react';

const styles = cssMap({
  root: {
    '&:focus, & *:focus': {
      outline: 'none',
      boxShadow: 'none',
    },
  },
});

export function Component() {
  return (
    <div css={styles.root}>
      <button type="button">Child</button>
    </div>
  );
}
