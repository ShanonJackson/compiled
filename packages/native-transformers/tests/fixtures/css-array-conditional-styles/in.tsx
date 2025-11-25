import { css, jsx } from '@compiled/react';

const fullpageStyles = css({
  borderTopWidth: '8px',
});

const modalStyles = css({
  marginTop: '-40px',
});

const customSpacingStyles = css({
  maxWidth: '1920px',
});

export default function Component({
  isEmbedView,
  isModalView,
}: {
  isEmbedView?: boolean;
  isModalView?: boolean;
}) {
  const customCss = [
    customSpacingStyles,
    isEmbedView !== true && isModalView !== true ? fullpageStyles : null,
    isModalView === true ? modalStyles : null,
  ];

  return <div css={customCss}>hello</div>;
}
