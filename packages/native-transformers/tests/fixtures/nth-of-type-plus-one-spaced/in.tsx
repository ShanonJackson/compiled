/** @jsx jsx */
import { css, jsx } from '@compiled/react';

// Reproduces spacing in nth-of-type selector that mismatches between Babel/SWC.
const statisticStyles = css({
	'&:nth-of-type(n + 1)': {
		paddingLeft: '8px',
	},
});

const Fixture = () => (
	<table>
		<tbody>
			<tr>
				<td css={statisticStyles}>first</td>
			</tr>
			<tr>
				<td css={statisticStyles}>second</td>
			</tr>
		</tbody>
	</table>
);

export default Fixture;
