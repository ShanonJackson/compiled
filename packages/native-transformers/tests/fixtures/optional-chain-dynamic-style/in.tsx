import { styled } from '@compiled/react';

type Position = { left?: number; top?: number };

// This mirrors the Jira toggle-buttons-old usage that triggered the crash: a styled
// component with dynamic properties that dereference optional chaining inside the
// arrow expression.
const Fieldset = styled.fieldset<{ position?: Position }>({
  position: 'relative',
  left: (props) => (props.position?.left ? `${props.position?.left}px` : 0),
  top: (props) => (props.position?.top ? `${props.position?.top}px` : 0),
});

export default function Component(props: { position?: Position }) {
  return <Fieldset position={props.position}>hello</Fieldset>;
}
