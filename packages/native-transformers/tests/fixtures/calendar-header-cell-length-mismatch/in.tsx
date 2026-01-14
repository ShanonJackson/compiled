/** @jsx jsx */
import { css, jsx, styled } from '@compiled/react';
import { token } from '@atlaskit/tokens';

const weekViewStyles = css({
	'.fc-theme-standard th .fc-col-header-cell-cushion>div': {
		'> div': {
			font: token('font.heading.xxsmall'),
			color: token('color.text.subtle'),
			fontWeight: token('font.weight.semibold'),
			width: 'auto',
			height: 'auto',
			paddingLeft: token('space.050'),
		},
		'> span': {
			font: token('font.heading.xxsmall'),
			color: token('color.text.subtlest'),
			fontWeight: token('font.weight.semibold'),
		},
		'> div > h4': {
			font: token('font.heading.xxsmall'),
			color: token('color.text.subtlest'),
			fontWeight: token('font.weight.semibold'),
		},
	},
	'.fc-theme-standard th .fc-day-today .fc-col-header-cell-cushion>div': {
		'> div': {
			color: token('color.text.brand'),
			background: 'none',
		},
		'> span': {
			color: token('color.text.brand'),
			background: 'none',
		},
		'> div > h4': {
			color: token('color.text.brand'),
			background: 'none',
		},
	},
});

const CalendarHeader = styled.div<{
	weekViewEnabled: boolean;
	isVisualRefresh?: boolean;
	hasIssuesPages?: boolean;
}>(
	({ weekViewEnabled }) => weekViewEnabled && weekViewStyles,
	{
		borderRadius: ({ hasIssuesPages, isVisualRefresh }) =>
			// match the original CalendarRenderer border radius branching
			hasIssuesPages
				? isVisualRefresh
					? `${token('space.075')} ${token('space.075')} 0px 0px`
					: '2px 2px 0px 0px'
				: isVisualRefresh
					? token('space.075')
					: token('radius.xsmall'),
		'.fc-theme-standard th': {
			'.fc-scrollgrid-sync-inner': {
				display: 'flex',
				flexDirection: 'row',
				justifyContent: 'center',
				alignItems: 'center',
				height: ({ isVisualRefresh }) => (isVisualRefresh ? '40px' : '24px'),
			},
			'.fc-col-header-cell-cushion': {
				paddingBottom: 0,
				' > div': {
					color: token('color.text.subtle'),
					font: token('font.heading.xxsmall'),
					fontWeight: token('font.weight.bold'),
					display: ({ weekViewEnabled }) => (weekViewEnabled ? 'flex' : 'block'),
					flexDirection: ({ weekViewEnabled }) => weekViewEnabled && 'row',
					textTransform: 'capitalize',
				},
				' > span': {
					color: token('color.text.subtle'),
					font: token('font.heading.xxsmall'),
					fontWeight: token('font.weight.bold'),
					display: ({ weekViewEnabled }) => (weekViewEnabled ? 'flex' : 'block'),
					flexDirection: ({ weekViewEnabled }) => weekViewEnabled && 'row',
					textTransform: 'capitalize',
				},
			},
		},
	},
);

const CalendarHeaderFixture = ({ weekViewEnabled = true }) => (
	<CalendarHeader weekViewEnabled={weekViewEnabled} isVisualRefresh>
		header
	</CalendarHeader>
);

export default CalendarHeaderFixture;
