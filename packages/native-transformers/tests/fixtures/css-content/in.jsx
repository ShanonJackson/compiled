import { css } from '@compiled/react';

export const blank = css({ content: '' });
export const text = css({ content: 'hello' });
export const quoted = css({ content: "'hello'" });
export const arrow = css({ content: '→' });


export const Element = <div css={[blank, text, quoted, arrow]}/>
