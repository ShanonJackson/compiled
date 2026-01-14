import { ClassNames, css, cssMap, keyframes, styled } from '@compiled/react';

const colors = {
  primary: 'tomato',
  secondary: '#daa520',
};

const fadeIn = keyframes`
  0% { opacity: 0; }
  to { opacity: 1; }
`;

export const dynamicText = css`
  color: ${colors.primary};
  background: linear-gradient(${['to right', 'to bottom'][0]}, ${colors.primary}, ${colors.secondary});
`;

export const FancyButton = styled.button`
  animation: ${fadeIn} 2s ease-in-out;
  border-width: 2px;
  border-style: solid;
  border-color: ${colors.secondary};
  &:hover {
    transform: scale(1.1);
  }
`;

const themed = cssMap({
  primary: {
    color: colors.primary,
  },
  secondary: {
    color: colors.secondary,
  },
});

const alias = 'secondary';
export const mappedClass = themed[alias];

export const WithClassNames = () => (
  <ClassNames>
    {({ css: cx }) => (
      <div className={cx({
        color: colors.secondary,
        padding: `${colors.primary.length * 2}px`,
      })}>
        example
      </div>
    )}
  </ClassNames>
);
