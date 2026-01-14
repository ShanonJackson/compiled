/** @jsx jsx */
import { jsx, styled } from '@compiled/react';

// Minimal reproduction of the header cell focus outline rules where the additional
// :focus::before override repeats some properties and currently produces a length
// mismatch between Babel and SWC outputs.
const FocusCell = styled.th(
  {
    position: 'relative',
    '&:focus': {
      outline: 'unset',
      borderRight: '2px solid currentColor',
      boxShadow: 'inset 0 2px 0 0 currentColor',
      '&:before': {
        content: ' ',
        display: 'block',
        position: 'absolute',
        height: '100%',
        width: '100%',
        top: '-3px',
        left: '-1px',
        right: '-1px',
        bottom: '-1px',
        border: '2px solid currentColor',
      },
    },
  },
  {
    ':focus::before': {
      top: '0px',
      right: '-1px',
      left: '-1px',
      bottom: '-1px',
      height: 'calc(100% - 2px)',
    },
  }
);

export default function Fixture() {
  return (
    <table>
      <thead>
        <tr>
          <FocusCell>Header</FocusCell>
        </tr>
      </thead>
    </table>
  );
}
