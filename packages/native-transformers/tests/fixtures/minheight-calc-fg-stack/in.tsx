import { styled } from '@compiled/react';
import { fg } from '@atlassian/jira-feature-gating';

const Comp = styled.div({
  minHeight: ({ isFlexible, isSwimlaneMode }: { isFlexible: boolean; isSwimlaneMode: boolean }) =>
    isFlexible && !isSwimlaneMode
      ? fg('avoid_board_scroll_container_style_changes')
        ? '100%'
        : 'calc(var(--board-scroll-element-height) * 1px - 8px)'
      : undefined,
});

export const Component = () => <Comp isFlexible isSwimlaneMode={false} />;
