/** @jsx jsx */
import { jsx, cssMap } from '@atlaskit/css';
import Textfield from '@atlaskit/textfield';
import { token } from '@atlaskit/tokens';

const textFieldStyle = cssMap({
  invalid: { color: token('color.text.danger') },
  valid: { color: token('color.text') },
});

type Props = {
  isValid: boolean;
};

export const Example = ({ isValid }: Props) => (
  <Textfield css={[isValid ? textFieldStyle.valid : textFieldStyle.invalid]} />
);
