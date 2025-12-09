
/** @jsx jsx */
import React, { useCallback, type SyntheticEvent, type ComponentPropsWithoutRef } from 'react';
import { styled, css, jsx } from '@compiled/react';
import isNil from 'lodash/isNil';
import { token } from '@atlaskit/tokens';
import { gridSize } from '@atlassian/jira-common-styles/src/main.tsx';
import { fg } from '@atlassian/jira-feature-gating';
import { useIntl } from '@atlassian/jira-intl';
import { FIELD_TYPE_MAP } from '@atlassian/jira-issue-analytics/src/services/update-issue-field/constants.tsx';
import { useFieldConfigWithoutRefetch } from '@atlassian/jira-issue-field-base/src/services/field-config-service/main.tsx';
import { READ_VIEW_CONTAINER_SELECTOR } from '@atlassian/jira-issue-field-inline-edit/src/styled.tsx';
import { AsyncLazyNumberFieldInlineEdit } from '@atlassian/jira-issue-field-number-inline-edit/src/ui/async.tsx';
import { NumberInlineEditErrorBoundary } from '@atlassian/jira-issue-field-number-inline-edit/src/ui/error-boundary/index.tsx';
import { READ_VIEW_CONTAINER_SELECTOR as originalReadViewContainer } from '@atlassian/jira-issue-field-original-estimate/src/common/constants.tsx';
import { STORY_POINTS_TYPE } from '@atlassian/jira-platform-field-config/src/index.tsx';
import {
IP_BOARD_HOURS_PLANNING_UNIT,
IP_BOARD_DAYS_PLANNING_UNIT,
} from '@atlassian/jira-portfolio-3-plan-increment-common/src/common/constants.tsx';
import { ContextualAnalyticsData } from '@atlassian/jira-product-analytics-bridge';
import { toIssueKey } from '@atlassian/jira-shared-types/src/general.tsx';
import {
defaultTimeTrackingOptions,
SECONDS_PER_HOUR,
} from '@atlassian/jira-time-tracking-formatter/src/constants.tsx';
import type { TimeTrackingOptions } from '@atlassian/jira-time-tracking-formatter/src/types.tsx';
import UFOSegment from '@atlassian/jira-ufo-segment/src/index.tsx';
import { EstimateFieldStatic } from '../../../../../../common/fields/estimate-field/static/index.tsx';
import { EstimateWrapper } from '../../../../../../common/fields/estimate-field/wrapper/index.tsx';
import { useEditableField } from '../../../../../../common/fields/use-editable-field/index.tsx';
import { useFireInvalidFieldConfigError } from '../../../../../../common/fields/use-fire-invalid-field-config-error/index.tsx';
import { timeTrackingConfigTransformer } from '../../../../../../common/utils/time-tracking/index.tsx';
import { PACKAGE_NAME } from '../../../../../../model/constants.tsx';
import { issueIncrementPlanningUpdate } from '../../../../../../state/actions/issue/update/index.tsx';
import { useBoardDispatch, useBoardSelector } from '../../../../../../state/index.tsx';
import { getPreventInlineEditing } from '../../../../../../state/selectors/board/board-selectors.tsx';
import { issueParentIdsSelector } from '../../../../../../state/selectors/issue-parent/index.tsx';
import { getPlanningUnit } from '../../../../../../state/selectors/software/software-selectors.tsx';
import { getTimeTrackingOptions } from '../../../../../../state/selectors/work/work-selectors.tsx';
import { useIsIncrementPlanningBoard } from '../../../../../../state/state-hooks/capabilities/index.tsx';
import { INLINE_EDITING_FIELD_ZINDEX } from '../constants.tsx';
import { STORY_POINT_WRAPPER_TEST_ID } from './constants.tsx';
import messages from './messages.tsx';
import type {
StoryPointFieldProps,
StoryPointFieldInnerProps,
StorypointsEstimateWrapperProps,
} from './types.tsx';

const stopPropagation = (e: SyntheticEvent<HTMLElement>) => e.stopPropagation();

const fieldKey = 'storyPoints';

const StoryPointFieldInner = ({
issueId,
issueKey,
storyPointFieldId,
estimate,
onFailure,
dialogPlacement,
showTooltip,
tooltipMessage,
editInputLabel,
...props
}: StoryPointFieldInnerProps) => {
const isIncrementPlanningBoard = useIsIncrementPlanningBoard();

	const { formatMessage } = useIntl();
	const dispatch = useBoardDispatch();

	const timeTrackingOptions: TimeTrackingOptions = useBoardSelector((state) =>
		getTimeTrackingOptions(state),
	);

	const planningUnit = useBoardSelector((state) => getPlanningUnit(state));

	const preventInlineEditing = useBoardSelector((state) => getPreventInlineEditing(state));

	const storyPointKey = storyPointFieldId || '';

	const { fireInvalidFieldConfigError } = useFireInvalidFieldConfigError();
	const onFailureFireError = useCallback(
		(error: Error) => {
			onFailure?.();
			fireInvalidFieldConfigError(error);
		},
		[fireInvalidFieldConfigError, onFailure],
	);

	return (
		<ContextualAnalyticsData
			attributes={{
				isInlineEditing: true,
				fieldKey,
				fieldType: 'number',
				origin: 'issueCard',
			}}
		>
			<StorypointsEstimateWrapper
				data-testid={STORY_POINT_WRAPPER_TEST_ID}
				onClick={stopPropagation}
				onKeyDown={stopPropagation} // Prevent Enter from opening issue when cross or tick is focused
				hasValue={!isNil(estimate)}
				disableClick={preventInlineEditing}
				isIncrementPlanningBoard={isIncrementPlanningBoard}
			>
				<AsyncLazyNumberFieldInlineEdit
					editButtonLabel={formatMessage(messages.editButtonLabel, {
						storyPoints: estimate,
					})}
					componentAnalyticsData={{
						fieldType: FIELD_TYPE_MAP[STORY_POINTS_TYPE],
						isInlineEditing: true,
					}}
					{...props}
					actionSubject="inlineEdit"
					issueKey={toIssueKey(issueKey)}
					fieldKey={storyPointKey}
					analyticsFieldKeyAlias="storyPoints"
					onFailure={onFailureFireError}
					dialogPlacement={dialogPlacement}
					showTooltip={showTooltip}
					tooltipMessage={tooltipMessage}
					label={editInputLabel}
					{...(isIncrementPlanningBoard && {
						min: 0,
						saveField: async (_: string, fieldId: string, fieldValue: number | string | null) => {
							const getFieldValue = () => {
								if (isNil(fieldValue) || fieldValue === '') {
									return fieldValue;
								}
								if (planningUnit === IP_BOARD_HOURS_PLANNING_UNIT) {
									return Number(fieldValue) * SECONDS_PER_HOUR;
								}
								if (planningUnit === IP_BOARD_DAYS_PLANNING_UNIT) {
									const workingHoursPerDay =
										timeTrackingConfigTransformer(timeTrackingOptions).hoursPerDay ||
										defaultTimeTrackingOptions.workingHoursPerDay;
									return Number(fieldValue) * workingHoursPerDay * SECONDS_PER_HOUR;
								}
								return fieldValue;
							};
							dispatch(
								issueIncrementPlanningUpdate({
									issueId,
									fieldId,
									fieldValue: getFieldValue(),
								}),
							);
							// to make ts happy
							return undefined;
						},
					})}
				/>
			</StorypointsEstimateWrapper>
		</ContextualAnalyticsData>
	);
};

export const StoryPointField = ({ shouldRenderRichField, ...props }: StoryPointFieldProps) => {
const { formatMessage } = useIntl();
const storyPointKey = props.storyPointFieldId || '';
const [{ value: storyPointFieldConfig }] = useFieldConfigWithoutRefetch(
props.issueKey,
storyPointKey,
);
const shouldRenderRich = Boolean(shouldRenderRichField && storyPointFieldConfig);

	const editableField = useEditableField({
		isExperienceAvailable: shouldRenderRich,
	});

	const isIncrementPlanningBoard = useIsIncrementPlanningBoard();
	const issueParents = useBoardSelector((state) => issueParentIdsSelector(state));
	const isIssueParent = !!issueParents && issueParents.includes(`${props.issueId}`);

	const fallback = props.estimate ? <EstimateFieldStatic value={props.estimate} /> : null;

	if (shouldRenderRich) {
		return (
			<NumberInlineEditErrorBoundary
				packageName={PACKAGE_NAME}
				fallback={fallback}
				onError={editableField.onError}
			>
				<UFOSegment name="ng-board.inline-edit.story-point-field">
					<StoryPointFieldInner
						{...props}
						{...editableField}
						showTooltip={isIncrementPlanningBoard && isIssueParent}
						tooltipMessage={formatMessage(
							fg('jira-issue-terminology-refresh-m3')
								? messages.tooltipForEpicEstimateIssueTermRefresh
								: messages.tooltipForEpicEstimate,
						)}
						editInputLabel={formatMessage(messages.ariaLabel)}
					/>
				</UFOSegment>
			</NumberInlineEditErrorBoundary>
		);
	}
	return fallback;
};

// prevents warning for usage of style composition - can be removed when https://product-fabric.atlassian.net/browse/APP-728 is done.

const CompiledPropsForwarder = ({
disableClick,
isIncrementPlanningBoard,
...rest
}: StorypointsEstimateWrapperProps & ComponentPropsWithoutRef<typeof EstimateWrapper>) => (
<EstimateWrapper {...rest} />
);

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled -- To migrate as part of go/ui-styling-standard
const StorypointsEstimateWrapper = styled(CompiledPropsForwarder)<StorypointsEstimateWrapperProps>(
{
/* stylelint-disable-next-line selector-type-case, selector-type-no-unknown */
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values, @atlaskit/ui-styling-standard/no-unsafe-values, @atlaskit/ui-styling-standard/no-nested-selectors -- Ignored via go/DSP-18766
[READ_VIEW_CONTAINER_SELECTOR]: {
whiteSpace: 'nowrap',
marginLeft: token('space.negative.025'),
marginBottom: 0,
marginTop: `calc(${token('space.negative.100')} * 0.125)`, // reverse the margin-top added from BacklogWrapper
marginRight: 0,
},
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values, @atlaskit/ui-styling-standard/no-unsafe-values -- Ignored via go/DSP-18766
height: gridSize * 3.5,
},
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
({ disableClick }) =>
disableClick &&
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
css({
pointerEvents: 'none',
}),
{
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-unsafe-values, @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
[`&:not(:has(${originalReadViewContainer}, ${READ_VIEW_CONTAINER_SELECTOR}))`]: {
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values, @atlaskit/ui-styling-standard/no-unsafe-values -- Ignored via go/DSP-18766
zIndex: INLINE_EDITING_FIELD_ZINDEX,
},
},
/**
* In Increment planning boards, the estimate field is displayed closer to the right edge of a card
* than in other boards, therefore when editing the estimate, the text field is inelegantly rendered
* overflowing the right edge of the card:
*               |
*           ____|___
*           |  13 ⇳|  <--
*           ￣￣|￣￣
* ______________|
*
* Therefore we increase the right margin to visually render the text field within the card edges.
*/
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
({ isIncrementPlanningBoard }) =>
isIncrementPlanningBoard &&
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
css({
// eslint-disable-next-line @atlaskit/ui-styling-standard/no-unsafe-values, @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
[`&:not(:has(${originalReadViewContainer}, ${READ_VIEW_CONTAINER_SELECTOR}))`]: {
marginRight: `calc(${token('space.600')} + ${token('space.100')})`,
},
}),
);
