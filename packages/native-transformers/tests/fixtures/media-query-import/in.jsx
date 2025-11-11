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
