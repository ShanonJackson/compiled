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
  color: 'teal',
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

export const spreadTemplate = css`
  border-radius: 4px;
  box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.1);
`;

export const SpreadExampleComponent = () => (
  <main css={[spreadExample, spreadTemplate]}>
    spread styles
  </main>
);
