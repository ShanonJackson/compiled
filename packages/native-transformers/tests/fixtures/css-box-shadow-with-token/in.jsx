/** @jsx jsx */
import { jsx, css } from '@compiled/react';
import { token } from '@atlaskit/tokens';

const border = css({ boxShadow: `inset 0 -1px 0 0 ${"var(--border-color, '#FFF')"}` });

const Component = () => (
  <div css={border}>
    <span>Content with border</span>
  </div>
);

export default Component;
