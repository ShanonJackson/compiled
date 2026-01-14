import { styled, css } from '@compiled/react';
import { token } from '@atlaskit/tokens';

const READ_VIEW_CONTAINER_SELECTOR =
  '[data-component-selector=jira-issue-field-inline-edit-read-view-container]';
const ORIGINAL_READ_VIEW_SELECTOR =
  '[data-component-selector=jira-issue-field-original-estimate-read-view-container]';

const Wrapper = styled.div(
  {
    [READ_VIEW_CONTAINER_SELECTOR]: {
      whiteSpace: 'nowrap',
      marginLeft: token('space.negative.025'),
      marginBottom: 0,
      marginTop: `calc(${token('space.negative.100')} * 0.125)`,
      marginRight: 0,
    },
    height: token('space.200'),
  },
  ({ disableClick }) =>
    disableClick &&
    css({
      pointerEvents: 'none',
    }),
  {
    [`&:not(:has(${ORIGINAL_READ_VIEW_SELECTOR}, ${READ_VIEW_CONTAINER_SELECTOR}))`]: {
      zIndex: token('elevation.surface'),
    },
  },
  ({ isIncrementPlanningBoard }) =>
    isIncrementPlanningBoard &&
    css({
      [`&:not(:has(${ORIGINAL_READ_VIEW_SELECTOR}, ${READ_VIEW_CONTAINER_SELECTOR}))`]: {
        marginRight: `calc(${token('space.600')} + ${token('space.100')})`,
      },
    })
);

export const Component = ({ disableClick, isIncrementPlanningBoard }) => (
  <Wrapper disableClick={disableClick} isIncrementPlanningBoard={isIncrementPlanningBoard} />
);
