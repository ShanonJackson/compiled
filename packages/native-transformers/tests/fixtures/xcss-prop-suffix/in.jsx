import { cssMap } from '@atlaskit/css';

const styles = cssMap({
  row: {
    whiteSpace: 'normal',
  },
});

const Table = (props) => <div {...props} />;

export const Component = () => <Table rowCellXcss={styles.row} />;
