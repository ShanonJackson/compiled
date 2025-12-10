import type { ReactNode } from 'react';
import { styled as styled2 } from '@compiled/react';
import Button from '@atlaskit/button';
import { token } from '@atlaskit/tokens';
import { visuallyHiddenStyles } from '@atlassian/jira-accessibility/src/common/ui/screenreader-text/index.tsx';
import { gridSize } from '@atlassian/jira-common-styles/src/main.tsx';
import { isVisualRefreshEnabled } from '@atlassian/jira-visual-refresh-rollout/src/feature-switch/index.tsx';
import {
CARD_ROW_HEIGHT,
COMPACT_CARD_ROW_HEIGHT,
EXTRA_CARD_ROW_HEIGHT,
} from '../../../common/constants/index.tsx';
import {
CHECKBOX_COMPONENT_SELECTOR,
IMAGE_SIZE,
KEY_COMPONENT_SELECTOR,
} from './card-contents/constants.tsx';
import { MENU_PLACEHOLDER_ID } from './constants.tsx';

// If we didn't use an extra local variable, Compiled would auto-assign the value to the first return value
export const getBgColor = (
isSelected: boolean,
isFlagged: boolean,
isVisualRefreshBeta?: boolean,
) => {
let bgColor: string = token('elevation.surface.raised');

	if (isFlagged && isSelected) {
		bgColor = isVisualRefreshBeta
			? token('color.background.accent.red.subtlest.pressed')
			: token('color.background.warning.pressed');
	} else if (isFlagged) {
		bgColor = isVisualRefreshBeta
			? token('color.background.accent.red.subtlest')
			: token('color.background.warning');
	} else if (isSelected) {
		bgColor = token('color.background.selected');
	}

	return bgColor;
};

const getHoverBgColor = (
isSelected: boolean,
isFlagged: boolean,
isVisualRefreshBeta?: boolean,
) => {
let hoverBgColor: string = token('color.background.neutral.subtle.hovered');

	if (isFlagged) {
		hoverBgColor = isVisualRefreshBeta
			? token('color.background.accent.red.subtlest.hovered')
			: token('color.background.warning.hovered');
	} else if (isSelected) {
		hoverBgColor = token('color.background.selected.hovered');
	}

	return hoverBgColor;
};

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled, @atlaskit/ui-styling-standard/no-exported-styles -- Ignored via go/DSP-18766
export const Key = styled2.a<{ isDone?: boolean }>({
font: token('font.body.UNSAFE_small'),
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles
fontWeight: () =>
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values
isVisualRefreshEnabled() ? token('font.weight.regular') : token('font.weight.semibold'),
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles
fontVariantNumeric: () =>
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values
isVisualRefreshEnabled() ? 'tabular-nums' : undefined,
color: token('color.text.subtle'),

	outline: 'none',
	marginTop: 0,
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	textDecoration: ({ isDone = false }) => `${isDone ? 'line-through' : 'none'} !important`,
	whiteSpace: 'nowrap',
});

// TODO remove when cleaning up backlog-type-icon-component feature gate
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled, @atlaskit/ui-styling-standard/no-exported-styles -- Ignored via go/DSP-18766
export const Img = styled2.img({
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values, @atlaskit/ui-styling-standard/no-unsafe-values -- Ignored via go/DSP-18766
width: `${IMAGE_SIZE}px`,
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values, @atlaskit/ui-styling-standard/no-unsafe-values -- Ignored via go/DSP-18766
height: `${IMAGE_SIZE}px`,
verticalAlign: 'text-bottom',
});

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled, @atlaskit/ui-styling-standard/no-exported-styles -- Ignored via go/DSP-18766
export const CheckboxContainer = styled2.div<{
shouldShowCheckbox: boolean;
}>({
width: '16px',
zIndex: 1 /* surface the element above the interaction layer so can be triggered */,
display: 'flex',
justifyContent: 'center',
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
visibility: ({ shouldShowCheckbox }) => (shouldShowCheckbox ? 'visible' : 'hidden'),
gridColumn: 'checkbox / span 1',
gridRow: 'card-detail / span 1',
});

// TODO remove when cleaning up backlog-type-icon-component feature gate
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled, @atlaskit/ui-styling-standard/no-exported-styles -- Ignored via go/DSP-18766
export const ImageContainer = styled2.div({
gridColumn: 'issue-type / span 1',
gridRow: 'card-detail / span 1',
});

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled, @atlaskit/ui-styling-standard/no-exported-styles -- Ignored via go/DSP-18766
export const KeyContainer = styled2.div({
position: 'relative',
zIndex: 1 /* surface the element above the interaction layer so tooltips can be triggered */,
gridColumn: 'issue-key / span 1',
gridRow: 'card-detail / span 1',
});

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-exported-styles, @atlaskit/ui-styling-standard/no-styled
export const ScreenReaderKey = styled2(Key)({
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-unsafe-selectors
'&:not(:focus)': {
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-unsafe-values, @atlaskit/ui-styling-standard/no-imported-style-values
...visuallyHiddenStyles,
},
'&:focus': {
outline: 'none',
boxShadow: `0 0 0 2px ${token('color.border.focused')}`,
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-nested-selectors, @atlaskit/ui-styling-standard/no-unsafe-values, @atlaskit/ui-styling-standard/no-imported-style-values
[`+ [data-component-selector="${KEY_COMPONENT_SELECTOR}"]`]: {
display: 'none',
},
},
'&:active': {
outline: 'none',
},
});

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled, @atlaskit/ui-styling-standard/no-exported-styles -- Ignored via go/DSP-18766
export const SummaryAndParentContainer = styled2.div<{ isSummaryFieldEditing?: boolean }>({
display: 'flex',
flex: 1,
boxSizing: 'border-box',
userSelect: 'none',
gridColumn: 'summary / span 1',
gridRow: 'card-detail / span 1',
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
position: ({ isSummaryFieldEditing }) => (isSummaryFieldEditing ? 'relative' : 'unset'),
alignItems: 'center',
minWidth: 0,
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-nested-selectors -- Ignored via go/DSP-18766
'& > *': {
minWidth: 0,
},
});

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled, @atlaskit/ui-styling-standard/no-exported-styles -- Ignored via go/DSP-18766
export const SummaryWrapper = styled2.div<{ isSummaryFieldEditing: boolean }>({
outline: 'none',
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
zIndex: ({ isSummaryFieldEditing }) =>
isSummaryFieldEditing
? 2
: 1 /* surface the element above the interaction layer so tooltips can be triggered */,
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
width: ({ isSummaryFieldEditing }) => isSummaryFieldEditing && '100%',
});

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled, @atlaskit/ui-styling-standard/no-exported-styles -- Ignored via go/DSP-18766
export const Summary = styled2.div({
'&::before': {
/* to avoid safari native tooltip */
content: "''",
display: 'block',
},

	color: token('color.text'),
	whiteSpace: 'nowrap',
	overflow: 'hidden',
	textOverflow: 'ellipsis',
});

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled, @atlaskit/ui-styling-standard/no-exported-styles -- Ignored via go/DSP-18766
export const EpicWrapper = styled2.div<{
widthMultiplier: number;
}>({
display: 'grid',
justifyItems: 'start',
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
gridTemplateColumns: ({ widthMultiplier }) =>
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
`repeat(auto-fit,${gridSize * widthMultiplier}px)`,
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles, @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
minWidth: ({ widthMultiplier }) => `${gridSize * widthMultiplier}px`,
padding: '0px',
});

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled, @atlaskit/ui-styling-standard/no-exported-styles -- Ignored via go/DSP-18766
export const VersionsContainer = styled2.div<{ widthMultiplier: number }>({
display: 'grid',
zIndex: '1',
overflow: 'hidden',
textOverflow: 'ellipsis',
alignItems: 'center',
justifyItems: 'start',
pointerEvents: 'none',
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
gridTemplateColumns: ({ widthMultiplier }) =>
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
`repeat(auto-fit,${gridSize * widthMultiplier}px)`,
});

// TODO remove on clean up TNK-570 - moved to card-contents/flag
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled, @atlaskit/ui-styling-standard/no-exported-styles -- Ignored via go/DSP-18766
export const FlagContainer = styled2.div({
position: 'relative',
zIndex: 1 /* surface the element above the interaction layer so tooltips can be triggered */,
});

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled, @atlaskit/ui-styling-standard/no-exported-styles -- Ignored via go/DSP-18766
export const StatusContainer = styled2.div<{ widthMultiplier: number }>({
display: 'grid',
height: '24px',
pointerEvents: 'none',
overflow: 'inherit',
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
gridTemplateColumns: ({ widthMultiplier }) =>
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
`repeat(auto-fit,${gridSize * widthMultiplier}px)`,
});

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled, @atlaskit/ui-styling-standard/no-exported-styles -- Ignored via go/DSP-18766
export const ExtraFieldsContainer = styled2.div<{
hideTypeIcon?: boolean;
isSmartCardEnabled: boolean;
}>({
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
gridColumn: ({ hideTypeIcon }) => (hideTypeIcon ? 'issue-key / end' : 'issue-type / end'),
gridRow: 'card-extra-fields / end',
display: 'grid',
boxSizing: 'border-box',
gridTemplateColumns: `minmax(auto, max-content) ${token('space.100')} minmax(auto, max-content) ${token('space.100')} minmax(
        auto,
        max-content
    )`,
alignItems: 'start',
color: token('color.text.subtlest'),
columnGap: token('space.050'),
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
height: (props) =>
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values
props.isSmartCardEnabled ? token('space.300') : `${EXTRA_CARD_ROW_HEIGHT}px`,
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles
paddingTop: (props) => (props.isSmartCardEnabled ? token('space.025') : token('space.0')),
overflow: 'hidden',
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-nested-selectors -- Ignored via go/DSP-18766
'& > *': {
minWidth: 0 /* required to allow field content truncation within the tooltip wrapper */,
zIndex: 1 /* surface the element above the interaction layer so tooltips can be triggered */,
},
});

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled, @atlaskit/ui-styling-standard/no-exported-styles -- Ignored via go/DSP-18766
export const AssigneeContainer = styled2.div({
gridColumn: 'assignee / span 1',
gridRow: 'card-detail / span 1',
});

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled, @atlaskit/ui-styling-standard/no-exported-styles -- Ignored via go/DSP-18766
export const CardsAmount = styled2.div({
position: 'absolute',
zIndex: 5,
top: 0,
right: 0,
transform: 'translate(50%, -50%)',
});

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled, @atlaskit/ui-styling-standard/no-exported-styles -- Ignored via go/DSP-18766
export const DragHandleCardsAmount = styled2.div({
position: 'absolute',
zIndex: 5,
top: token('space.negative.050'),
left: token('space.negative.050'),
transform: 'translate(50%, -50%)',
});

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled, @atlaskit/ui-styling-standard/no-exported-styles -- Ignored via go/DSP-18766
export const StyledSpinner = styled2.div({
gridColumn: 'menu / span 1',
gridRow: 'card-detail / span 1',
});

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled, @atlaskit/ui-styling-standard/no-exported-styles -- Ignored via go/DSP-18766
export const OptionalFieldsContainer = styled2.div({
gridTemplateColumns: 'repeat(auto-fill, min-content)',
gridColumn: 'optional-fields / span 1',
gridRow: 'card-detail / span 1',
display: 'flex',
boxSizing: 'border-box',
alignItems: 'center',
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-nested-selectors -- Ignored via go/DSP-18766
'& > *': {
marginTop: 0,
marginRight: token('space.050'),
marginBottom: 0,
marginLeft: token('space.050'),
},
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-nested-selectors, @atlaskit/ui-styling-standard/no-unsafe-selectors -- Ignored via go/DSP-18766
'& > :first-of-type': {
marginLeft: 0,
},
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-nested-selectors, @atlaskit/ui-styling-standard/no-unsafe-selectors -- Ignored via go/DSP-18766
'& > :last-of-type': {
marginRight: 0,
},
});

export const getCardContainerCheckboxGridColumnValue = () => '[start checkbox] min-content ';

export const getCardContainerChevronGridColumnValue = (
gridTemplateColumnsValue: string,
shouldShowChevron: boolean | undefined,
) => {
if (shouldShowChevron) {
if (gridTemplateColumnsValue) {
return '[chevron] 20px';
}
return '[start chevron] 12px ';
}
return '';
};

export const getCardContainerIssueTypeGridColumnValue = (gridTemplateColumnsValue: string) => {
if (gridTemplateColumnsValue) {
return '[issue-type] 16px ';
}
return '[start issue-type] 16px ';
};

export const getCardContainerPadding = ({ isOptimistic }: { isOptimistic?: boolean }) => {
if (isOptimistic) {
return `0 ${gridSize / 2}px 0 ${gridSize * 3.5}px`;
}

	return `0 ${gridSize / 2}px 0 ${gridSize * 1.5}px`;
};

type CardContainerProps = {
shouldShowChevron?: boolean;
isCompact?: boolean;
isSelected: boolean;
isFlagged: boolean;
lowerOpacity: boolean;
isDraggable: boolean;
isDragHandle: boolean;
children: ReactNode;
hasExtraFields: boolean;
isOptimistic?: boolean;
hideTypeIcon?: boolean;
isAssigneeShown?: boolean;
isVisualRefreshBeta?: boolean;
};

const calculateGridTemplateValues = ({
hasExtraFields,
shouldShowChevron,
isDragHandle,
isCompact = false,
hideTypeIcon = false,
isAssigneeShown = true,
}: Partial<CardContainerProps>): {
gridTemplateColumnsValue: string;
gridTemplateRowsValue: string;
} => {
let gridTemplateColumnsValue = '';
let gridTemplateRowsValue = '';

	gridTemplateColumnsValue += getCardContainerCheckboxGridColumnValue();
	gridTemplateColumnsValue += getCardContainerChevronGridColumnValue(
		gridTemplateColumnsValue,
		shouldShowChevron,
	);
	if (!hideTypeIcon)
		gridTemplateColumnsValue += getCardContainerIssueTypeGridColumnValue(gridTemplateColumnsValue);

	gridTemplateColumnsValue += '[issue-key] min-content [summary] 1fr';
	if (!isDragHandle) {
		gridTemplateColumnsValue += ' ';
		gridTemplateColumnsValue += '[card-group-key] max-content ';
		gridTemplateColumnsValue += [
			'[optional-fields] max-content',
			/* These values have to be in PX to ensure the elements take exactly the amount of space they need. Using a token/REM causes issues if browser's font size settings are set to smaller or larger */
			`[assignee] ${isAssigneeShown ? 'calc(8px * 3.5)' : token('space.0')}`,
			'[menu] 32px',
			'[end]',
		].join(' ');
	}

	// Minus 1 to fix layout issues
	// We need to totally rework the layout to do this properly
	gridTemplateRowsValue += `[start card-detail] ${
		(isCompact ? COMPACT_CARD_ROW_HEIGHT : CARD_ROW_HEIGHT) - 1
	}px `;
	if (!isDragHandle && hasExtraFields) {
		gridTemplateRowsValue += `[card-extra-fields] ${EXTRA_CARD_ROW_HEIGHT}px [end]`;
	} else {
		gridTemplateRowsValue += '[end]';
	}

	return {
		gridTemplateColumnsValue,
		gridTemplateRowsValue,
	};
};

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled, @atlaskit/ui-styling-standard/no-exported-styles -- Ignored via go/DSP-18766
export const CardContainer = styled2.div<CardContainerProps>({
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
backgroundColor: ({ isSelected = false, isFlagged = false, isVisualRefreshBeta }) =>
getBgColor(isSelected, isFlagged, isVisualRefreshBeta),
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
cursor: ({ isDraggable }) => (isDraggable === true ? 'pointer' : 'default'),
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
opacity: ({ lowerOpacity }) => (lowerOpacity ? 0.4 : 'inherit'),

	display: 'grid',
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	gridTemplateColumns: ({
		hasExtraFields,
		shouldShowChevron,
		isDragHandle,
		isCompact = false,
		hideTypeIcon = false,
		isAssigneeShown = true,
	}: CardContainerProps) =>
		calculateGridTemplateValues({
			hasExtraFields,
			shouldShowChevron,
			isDragHandle,
			isCompact,
			hideTypeIcon,
			isAssigneeShown,
		}).gridTemplateColumnsValue,
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	gridTemplateRows: ({
		hasExtraFields,
		shouldShowChevron,
		isDragHandle,
		isCompact = false,
		hideTypeIcon = false,
		isAssigneeShown = true,
	}: CardContainerProps) =>
		calculateGridTemplateValues({
			hasExtraFields,
			shouldShowChevron,
			isDragHandle,
			isCompact,
			hideTypeIcon,
			isAssigneeShown,
		}).gridTemplateRowsValue,
	columnGap: token('space.050'),
	rowGap: 0,

	alignItems: 'center',
	position: 'relative',
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	height: ({ hasExtraFields, isCompact = false }) => {
		// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
		const cardRowHeight = isCompact ? COMPACT_CARD_ROW_HEIGHT : CARD_ROW_HEIGHT;
		// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
		return hasExtraFields ? `${cardRowHeight + EXTRA_CARD_ROW_HEIGHT}px` : `${cardRowHeight}px`;
	},
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	padding: ({ isOptimistic }) =>
		getCardContainerPadding({
			isOptimistic,
		}),
	borderWidth: token('border.width'),
	borderStyle: 'solid',
	borderColor: token('color.border'),
	textDecoration: 'none',
	marginTop: token('space.negative.025'),

	'&:hover': {
		// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
		backgroundColor: ({ isSelected = false, isFlagged = false, isVisualRefreshBeta }) =>
			getHoverBgColor(isSelected, isFlagged, isVisualRefreshBeta),
		// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
		'--jsw-card-background-color': ({ isSelected = false, isFlagged = false }) =>
			getHoverBgColor(isSelected, isFlagged),
		// eslint-disable-next-line @atlaskit/ui-styling-standard/no-unsafe-values, @atlaskit/ui-styling-standard/no-nested-selectors, @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
		[`[data-component-selector="${MENU_PLACEHOLDER_ID}"]`]: {
			opacity: 1,
			visibility: 'visible',
		},

		// eslint-disable-next-line @atlaskit/ui-styling-standard/no-unsafe-values, @atlaskit/ui-styling-standard/no-nested-selectors, @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
		[`[data-component-selector="${CHECKBOX_COMPONENT_SELECTOR}"]`]: {
			visibility: 'visible',
		},
	},

	'&:focus, &:focus-within': {
		// eslint-disable-next-line @atlaskit/ui-styling-standard/no-unsafe-values, @atlaskit/ui-styling-standard/no-nested-selectors, @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
		[`[data-component-selector="${MENU_PLACEHOLDER_ID}"]`]: {
			opacity: 1,
			visibility: 'visible',
		},
		// eslint-disable-next-line @atlaskit/ui-styling-standard/no-unsafe-values, @atlaskit/ui-styling-standard/no-nested-selectors, @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
		[`[data-component-selector="${CHECKBOX_COMPONENT_SELECTOR}"]`]: {
			visibility: 'visible',
		},
	},

	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	'--jsw-card-background-color': ({ isSelected = false, isFlagged = false }) =>
		getBgColor(isSelected, isFlagged),
});

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled, @atlaskit/ui-styling-standard/no-exported-styles -- Ignored via go/DSP-18766
export const ContentContainer = styled2.div({
outline:
'none' /* NOTE: This is needed so we won't have the browser bulit-in focus ring when inline editor in cards lost focus in FireFox or Safari */,
backgroundColor: token('elevation.surface.raised'),
});

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled, @atlaskit/ui-styling-standard/no-exported-styles -- Ignored via go/DSP-18766
export const CardGroupKey = styled2(Key)({
position: 'inherit',
display: 'contents',
});

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled, @atlaskit/ui-styling-standard/no-exported-styles -- Ignored via go/DSP-18766
export const VersionIconWrapper = styled2.span({
display: 'flex',
flexDirection: 'initial',
});

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled, @atlaskit/ui-styling-standard/no-exported-styles -- Ignored via go/DSP-18766
export const VersionIconNumber = styled2.span({
position: 'relative',
color: token('color.text.subtlest'),
font: token('font.body.small'),
fontWeight: token('font.weight.bold'),
paddingLeft: token('space.025'),
});

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled, @atlaskit/ui-styling-standard/no-exported-styles -- Ignored via go/DSP-18766
export const AddFlagButton = styled2(Button)({
opacity: '0',
transition: 'opacity .35s ease',
'&:hover': {
opacity: '1',
background: 'none',
},

	marginTop: token('space.negative.050'),
	marginRight: token('space.negative.050'),
	marginBottom: token('space.negative.050'),
	marginLeft: token('space.negative.050'),
});

// Remove as part of backlog_tooltip_content_not_accessible FG cleanup
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled, @atlaskit/ui-styling-standard/no-exported-styles -- Ignored via go/DSP-18766
export const SubtaskIconWrapper = styled2.div({
zIndex: 1 /* surface the element above the interaction layer so tooltips can be triggered */,
});

// empty column for the cards without due date
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled, @atlaskit/ui-styling-standard/no-exported-styles -- Ignored via go/DSP-18766
export const CardDueDateWrapper = styled2.div<{ shouldRenderColumn: boolean; hasDueDate: boolean }>(
{
display: 'flex',
},
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
(props) =>
props.shouldRenderColumn
? {
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
minWidth: `${props.hasDueDate ? `${gridSize * 7.5}px` : `${gridSize * 8}px`}`,
}
: {},
);
