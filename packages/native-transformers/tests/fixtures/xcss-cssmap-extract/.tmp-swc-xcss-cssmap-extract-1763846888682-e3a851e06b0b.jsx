import { cssMap } from '@atlaskit/css';
import { Box } from '@atlaskit/primitives/compiled';

const styles = cssMap({
  container: {
    width: '300px',
  },
});

export const Component = () => <Box xcss={styles.container} />;
