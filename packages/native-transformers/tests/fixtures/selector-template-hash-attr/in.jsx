import { styled } from '@compiled/react';
import { token } from '@atlaskit/tokens';

const READ_VIEW_CONTAINER_SELECTOR =
  '[data-component-selector=jira-issue-field-inline-edit-read-view-container]';

const Wrapper = styled.div({
  [`&:not(:has(${READ_VIEW_CONTAINER_SELECTOR}, ${READ_VIEW_CONTAINER_SELECTOR}))`]: {
    marginRight: `calc(${token('space.600')} + ${token('space.100')})`,
  },
});

export const Component = () => <Wrapper />;
