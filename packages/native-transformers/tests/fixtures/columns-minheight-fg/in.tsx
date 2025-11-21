import React from 'react';
import { styled as styled2, jsx } from '@compiled/react';
import { fg } from '@atlassian/jira-feature-gating';

const Comp = styled2.div({
  minHeight: ({ isFlexible, isSwimlaneMode }: { isFlexible: boolean; isSwimlaneMode: boolean }) =>
    isFlexible && !isSwimlaneMode
      ? fg('flag') && 'calc(var(--board-scroll-element-height) * 1px - 8px)'
      : undefined,
});

export const Component = () => <Comp isFlexible isSwimlaneMode={false} />;
