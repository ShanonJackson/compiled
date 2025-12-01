import { styled } from '@compiled/react';

const Button = styled.button({
	cursor: ({ isDisabled, isFromServerSide }) => {
		if (isFromServerSide) {
			return { cursor: 'not-allowed' };
		}

		return isDisabled ? { cursor: 'default' } : 'pointer';
	},
});

export const Component = ({ isDisabled, isFromServerSide }) => (
	<Button isDisabled={isDisabled} isFromServerSide={isFromServerSide}>
		Assignee
	</Button>
);
