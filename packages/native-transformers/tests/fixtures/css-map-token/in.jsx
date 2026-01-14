import { cssMap } from '@compiled/react';

const styles = cssMap({
  root: {
    paddingBlockStart: "var(--space-0, 4px)",
    paddingBlockEnd: "var(--space-0, 4px)",
  },
});

export const Component = () => <div className={styles.root}>Hi</div>;
