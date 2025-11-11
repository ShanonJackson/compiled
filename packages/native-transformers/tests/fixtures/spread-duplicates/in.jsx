import { css } from '@compiled/react';

const base = {
  fontSize: '16px',
  color: 'purple',
  padding: '8px',
};

const overrides = {
  color: 'purple',
  padding: '10px',
  border: '1px solid black',
};

const conditional = {
  ...(true ? { color: 'teal' } : {}),
  backgroundColor: 'beige',
};

export const spreadExample = css({
  display: 'flex',
  ...base,
  gap: '12px',
  ...overrides,
  ...conditional,
  color: 'maroon',
});
