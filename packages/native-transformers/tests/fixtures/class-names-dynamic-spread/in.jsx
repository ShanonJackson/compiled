import { ClassNames } from '@compiled/react';
import { token } from '@atlaskit/tokens';

const TableWrapper = ({ selectedRows }) => {
	const styles = {};

	selectedRows.forEach((index) => {
		styles[`tbody tr:nth-child(${index + 1})`] = {
			backgroundColor: token('color.background.selected'),
		};
	});

	return (
		<ClassNames>
			{({ css }) => (
				<div
					className={css({
						position: 'relative',
						th: { height: '100%', verticalAlign: 'middle' },
						...styles,
					})}
				>
					content
				</div>
			)}
		</ClassNames>
	);
};

export const Fixture = () => <TableWrapper selectedRows={[0, 2]} />;
