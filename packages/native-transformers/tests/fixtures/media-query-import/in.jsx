import { css } from '@compiled/react';
import { DARK_MODE, MEDIA_QUERY } from './media';

export const responsiveStyles = css({
  color: 'black',
  [MEDIA_QUERY]: {
    color: 'royalblue',
    fontWeight: 'bold',
  },
  [DARK_MODE]: {
    color: 'white',
  },
});

export const responsiveTemplate = css`
  color: midnightblue;
  ${MEDIA_QUERY} {
    color: rebeccapurple;
  }
  ${DARK_MODE} {
    color: lavender;
  }
`;

export const MediaExample = () => (
  <section className={[responsiveStyles, responsiveTemplate].join(' ')}>
    responsive media queries
  </section>
);
