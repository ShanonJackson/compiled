import { css } from '@compiled/react';
export const Component = ()=>(<>
    // @compiled-disable-next-line transform-css-prop
    <div css={{
        color: 'red'
    }}/>
    <span css={{
        fontSize: 12
    }}/> // @compiled-disable-line transform-css-prop
  </>);
