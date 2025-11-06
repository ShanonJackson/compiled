import { cssMap } from '@compiled/react';

const styles = cssMap({
  primary: { color: 'red' },
  secondary: { color: 'blue' },
});

<Component xcss={isPrimary ? styles.primary : styles.secondary} />;
