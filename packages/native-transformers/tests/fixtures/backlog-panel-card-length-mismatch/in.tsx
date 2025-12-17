/** @jsx jsx */
import React, {
	createRef,
	useCallback,
	useEffect,
	useState,
	type ComponentType,
	type MouseEvent,
	type MutableRefObject,
	type ReactNode,
	type FocusEvent,
	useRef,
} from 'react';
import { css, styled, jsx, type CssFunction } from '@compiled/react';
import noop from 'lodash/noop';
import type { UIAnalyticsEvent } from '@atlaskit/analytics-next';
import Heading from '@atlaskit/heading';
import { Box, Flex, xcss } from '@atlaskit/primitives';
import Spinner from '@atlaskit/spinner';
import { token } from '@atlaskit/tokens';
import { media } from './responsive';
import { componentWithCondition } from './component-with-condition';
import { fireUIAnalytics } from './analytics';
import { isVisualRefreshEnabled } from './feature-switch';
import { useSoftwareProjectTheming } from './software-theme';
import { Tokens } from './custom-theme-constants';
import { getIsSoftwareThemingM2Enabled } from './theming-enabled';
import { fg } from './feature-gating';
import { useModalDialogActions } from './modal-dialog-actions';
import { AnnouncerV2 } from './announcer-v2';
import { useCardOpen } from './use-card-open';
import { cardColor } from './constants.tsx';
import Expand from './expand/index.tsx';
import type { CardContainerPropsWithDragging, CardPropsWithDraggingAndStates } from './types.tsx';
import { ContextMenu } from './context-menu/index.tsx';

const Container = ({
	innerRef,
	isActive = false,
	isDraggingOver = false,
	children,
	onClick = noop,
	isLoading,
	isDragging = false,
	expanded = false,
	onHoverOrFocus,
	onUnhoverOrBlur,
}: CardContainerPropsWithDragging) => {
	const containerProps = {
		'data-test-id': 'software-backlog-panel.common.ui.card.container',
		'data-testid': 'software-backlog-panel.common.ui.card.container',
	};
	const { isSoftwareFullTheming } = getIsSoftwareThemingM2Enabled()
		? // eslint-disable-next-line react-hooks/react-compiler
			// eslint-disable-next-line react-hooks/rules-of-hooks
			useSoftwareProjectTheming()
		: { isSoftwareFullTheming: false };

	const cardRef = createRef<HTMLDivElement>();
	useEffect(() => {
		if (cardRef && cardRef.current && cardRef.current.scrollIntoView && isLoading === true) {
			cardRef.current.scrollIntoView({ behavior: 'smooth' });
		}
	}, [cardRef, isLoading]);

	// This is to prevent onUnhoverOrBlur from being triggered when focused on children
	const onBlur = useCallback(
		(event: FocusEvent) => {
			if (innerRef?.current && !innerRef.current.contains(event.relatedTarget) && onUnhoverOrBlur) {
				onUnhoverOrBlur();
			}
		},
		[innerRef, onUnhoverOrBlur],
	);

	return isLoading === true ? (
		<StaticContainer ref={cardRef} {...containerProps} css={responsiveStyles}>
			{children}
		</StaticContainer>
	) : (
		<InteractiveContainer
			{...containerProps}
			ref={innerRef}
			isActive={isActive}
			isDraggingOver={isDraggingOver}
			onClick={onClick}
			isDragging={isDragging}
			expanded={expanded}
			onMouseEnter={onHoverOrFocus}
			onFocus={onHoverOrFocus}
			onBlur={onBlur}
			onMouseLeave={onUnhoverOrBlur}
			isSoftwareFullTheming={isSoftwareFullTheming}
			css={responsiveStyles}
		>
			{children}
		</InteractiveContainer>
	);
};

const Card = (props: CardPropsWithDraggingAndStates) => {
	const { isContentExpanded, toggleOpen } = useCardOpen(props.id);
	const { isSoftwareFullTheming } = getIsSoftwareThemingM2Enabled()
		? // eslint-disable-next-line react-hooks/react-compiler
			// eslint-disable-next-line react-hooks/rules-of-hooks
			useSoftwareProjectTheming()
		: { isSoftwareFullTheming: false };

	const {
		title = '',
		subtitle = null,
		children,
		isLoading,
		isActive = false,
		itemName = '',
		itemType,
		editButton,
		id,
	} = props;

	const content = typeof children === 'function' ? children(isContentExpanded) : children;
	const shouldShowContents = Boolean(content) && !isLoading;
	const [isHovered, setIsHovered] = useState(false);
	const menuTriggerRef = useRef<HTMLButtonElement>(null);
	const { setReturnFocusTo } = useModalDialogActions();

	const onToggleExpanded = (
		event: MouseEvent<HTMLElement>,
		analyticsEvent: UIAnalyticsEvent,
	): void => {
		// stop event propagation so that the card's isActive state does not get toggled
		event.stopPropagation();
		fireUIAnalytics(analyticsEvent, 'expandToggleButton', { expandState: !isContentExpanded });
		toggleOpen();
	};

	const [announcement, setAnnouncement] = useState('');

	const showEditButton = useCallback(() => setIsHovered(true), []);

	const hideEditButton = useCallback(() => setIsHovered(false), []);

	const handleMenuOpenChange = useCallback(
		(isOpen: boolean) => {
			if (isOpen && menuTriggerRef?.current) {
				setReturnFocusTo(menuTriggerRef);
			}
		},
		[setReturnFocusTo],
	);

	return (
		<Container
			{...props}
			expanded={isContentExpanded}
			onHoverOrFocus={showEditButton}
			onUnhoverOrBlur={hideEditButton}
		>
			<Header showChevron={shouldShowContents} visualRefresh={isVisualRefreshEnabled()}>
				{shouldShowContents && (
					<Expand
						data-testid="software-backlog-panel.common.ui.card.expand"
						isExpanded={isContentExpanded}
						onToggle={onToggleExpanded}
						itemName={itemName}
						itemType={itemType}
						isActive={isActive}
					/>
				)}
				<Title
					data-testid="software-backlog-panel.common.ui.card.title"
					visualRefresh={isVisualRefreshEnabled()}
				>
					{isVisualRefreshEnabled() &&
						(fg('jfp-a11y-team_fix_heading_on_backlog_card_ui') ? (
							<Flex justifyContent="space-between" alignItems="center">
								<Box
									as="h3"
									paddingBlockStart="space.0"
									xcss={[
										titleContainerStylesNew,
										isActive && titleSelectedStyles,
										isSoftwareFullTheming && isActive && titleSelectedThemedStyles,
									]}
								>
									{title}
								</Box>
								{isHovered && editButton}
							</Flex>
						) : (
							<Flex
								as="ul"
								justifyContent="space-between"
								alignItems="center"
								xcss={[
									titleContainerStyles,
									isActive && titleSelectedStyles,
									isSoftwareFullTheming && isActive && titleSelectedThemedStyles,
								]}
							>
								<Box as="li" paddingBlockStart="space.0">
									{title}
								</Box>
								{isHovered && editButton}
							</Flex>
						))}
					{!isVisualRefreshEnabled() && (
						<Heading as="h3" size="xsmall">
							{title}
						</Heading>
					)}
				</Title>
				{isLoading === true && (
					<Box xcss={cardSpinnerStyles}>
						<Spinner
							testId="software-backlog-panel.common.ui.card.spinner"
							size="small"
							appearance={isActive ? 'invert' : 'inherit'}
						/>
					</Box>
				)}
				{itemType === 'epic' && shouldShowContents && (
					<>
						<AnnouncerV2
							message={announcement}
							shouldAnnounce={announcement !== ''}
							liveMode="polite"
						/>
						<ContextMenu
							cardId={id}
							onOpenChange={handleMenuOpenChange}
							triggerRef={menuTriggerRef}
							setAnnouncement={setAnnouncement}
						/>
					</>
				)}
			</Header>
			{subtitle}
			{shouldShowContents && isContentExpanded && (
				<AnimatedHeight
					data-testid="software-backlog-panel.common.ui.card.content-container"
					aria-hidden={!isContentExpanded}
					isExpanded={isContentExpanded}
				>
					<Box xcss={contentStyles}>{content}</Box>
				</AnimatedHeight>
			)}
		</Container>
	);
};

export default Card;

const cardMaxWidth = 270;

const responsiveStyles = css({
	[media.below.lg]: {
		paddingTop: token('space.050'),
		paddingRight: token('space.100'),
		paddingBottom: token('space.050'),
		paddingLeft: token('space.100'),
	},
	[media.above.lg]: {
		paddingTop: token('space.100'),
		paddingRight: token('space.150'),
		paddingBottom: token('space.100'),
		paddingLeft: token('space.150'),
	},
});

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled -- To migrate as part of go/ui-styling-standard
const AnimatedHeight = styled.div<{
	isExpanded: boolean;
	children?: ReactNode;
}>({
	overflow: 'hidden',
	// eslint-disable-next-line @typescript-eslint/no-explicit-any, @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	maxHeight: (props: any) => (props.isExpanded ? '999px' : '0'),
	transition: 'max-height 0.3s ease-out',
});

const cardSpinnerStyles = xcss({
	marginLeft: 'space.150',
});

const defaultBoxShadow = token('elevation.shadow.raised');
const focusBoxShadow = `inset 0 0 0 2px ${token('color.border.focused')};`;

const outlineStyles = `
    outline: none;
    &:focus {
        box-shadow: ${focusBoxShadow};
    }
`;

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled -- To migrate as part of go/ui-styling-standard
const StaticContainer = styled.div<{
	['data-test-id']: string;
	isLoading?: boolean;
	isDraggingOver?: boolean;
	isDragging?: boolean;
	children?: ReactNode;
	innerRef?: {
		current: null | HTMLDivElement;
	};
}>({
	// eslint-disable-next-line @typescript-eslint/no-explicit-any, @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	cursor: (props: any) => (props.isLoading ? 'progress' : 'default'),
	display: 'flex',
	flexDirection: 'column',
	boxSizing: 'border-box',
	// eslint-disable-next-line @atlaskit/design-system/no-unsafe-design-token-usage -- The token value "4px" and fallback "3px" do not match and can't be replaced automatically.
	borderRadius: token('radius.small', '3px'),
	// eslint-disable-next-line @typescript-eslint/no-explicit-any, @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	boxShadow: (props: any) => (props.isDraggingOver ? focusBoxShadow : defaultBoxShadow),
	maxWidth: `${cardMaxWidth}px`,
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values, @atlaskit/ui-styling-standard/no-unsafe-values -- Ignored via go/DSP-18766
	backgroundColor: cardColor.default.mouseLeave.backgroundColor,
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values, @atlaskit/ui-styling-standard/no-unsafe-values -- Ignored via go/DSP-18766
	color: cardColor.default.mouseLeave.textColor,
	transition: 'background-color 140ms ease-in-out, color 140ms ease-in-out',
	marginTop: token('space.025'),
	marginRight: token('space.050'),
	marginBottom: token('space.025'),
	marginLeft: token('space.050'),
});

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled -- To migrate as part of go/ui-styling-standard
const StaticContainerNew = styled.div<{
	['data-test-id']: string;
	isLoading?: boolean;
	isDraggingOver?: boolean;
	isDragging?: boolean;
	children?: ReactNode;
	innerRef?: {
		current: null | HTMLDivElement;
	};
}>({
	// eslint-disable-next-line @typescript-eslint/no-explicit-any, @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	cursor: (props: any) => (props.isLoading ? 'progress' : 'default'),
	display: 'flex',
	flexDirection: 'column',
	boxSizing: 'border-box',
	borderRadius: token('radius.small'),
	maxWidth: `${cardMaxWidth}px`,
	backgroundColor: 'transparent',
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values, @atlaskit/ui-styling-standard/no-unsafe-values -- Ignored via go/DSP-18766
	color: cardColor.default.mouseLeave.textColor,
	transition: 'background-color 140ms ease-in-out, color 140ms ease-in-out',
	marginTop: token('space.025'),
	marginRight: token('space.100'),
	marginBottom: token('space.025'),
	marginLeft: token('space.100'),
});

const InteractiveContainerOld: ComponentType<{
	['data-test-id']: string;
	ref?: MutableRefObject<HTMLDivElement | null>;
	isDragging?: boolean;
	isActive?: boolean;
	isDraggingOver: boolean;
	tabIndex?: number;
	onClick?: (event: MouseEvent<HTMLElement>) => void;
	onMouseDown?: (event: MouseEvent<HTMLElement>) => void;
	children?: ReactNode;
	isSoftwareFullTheming?: boolean;
	css?: CssFunction | CssFunction[];
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled -- To migrate as part of go/ui-styling-standard
}> = styled(StaticContainer)(
	{
		'&:hover, &:focus-within': {
			// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values, @atlaskit/ui-styling-standard/no-unsafe-values -- Ignored via go/DSP-18766
			backgroundColor: cardColor.default.mouseOver.backgroundColor,
			// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values, @atlaskit/ui-styling-standard/no-unsafe-values -- Ignored via go/DSP-18766
			color: cardColor.default.mouseOver.textColor,
		},
	},
	{
		'&:active': {
			// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
			'background-color': ({ isActive }) =>
				isActive
					? // eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
						cardColor.active.mouseDown.backgroundColor
					: // eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
						cardColor.default.mouseDown.backgroundColor,
		},
	},
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	(props) =>
		props.isDragging && {
			'&, &:hover': {
				'background-color': token('color.background.selected'),
			},
		},
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	(props) =>
		props.isActive && {
			'&': {
				// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
				'background-color': cardColor.active.mouseLeave.backgroundColor,
				// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
				color: cardColor.active.mouseLeave.textColor,
			},
			'&:hover, &:focus-within': {
				// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
				'background-color': cardColor.active.mouseOver.backgroundColor,
				// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
				color: cardColor.active.mouseOver.textColor,
			},
		},
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	(props) =>
		props.isDraggingOver && {
			'&, &:hover': {
				'background-color': token('color.background.selected'),

				color: token('color.text'),
			},
		},
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-unsafe-values -- Ignored via go/DSP-18766
	outlineStyles,
);

const InteractiveContainerNew: ComponentType<{
	['data-test-id']: string;
	ref?: MutableRefObject<HTMLDivElement | null>;
	isDragging?: boolean;
	isActive?: boolean;
	isDraggingOver: boolean;
	tabIndex?: number;
	onClick?: (event: MouseEvent<HTMLElement>) => void;
	onMouseDown?: (event: MouseEvent<HTMLElement>) => void;
	onMouseEnter?: (event: MouseEvent<HTMLElement>) => void;
	onFocus?: (event: FocusEvent) => void;
	onBlur?: (event: FocusEvent) => void;
	onMouseLeave?: (event: MouseEvent<HTMLElement>) => void;
	children?: ReactNode;
	expanded?: boolean;
	isSoftwareFullTheming?: boolean;
	css?: CssFunction | CssFunction[];
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled -- To migrate as part of go/ui-styling-standard
}> = styled(StaticContainerNew)(
	{
		borderColor: 'transparent',
		borderStyle: 'solid',
		borderWidth: token('border.width'),
		borderRadius: token('radius.small'),
		'&:hover, &:focus-within': {
			// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- needed for theming
			backgroundColor: ({ isSoftwareFullTheming }) =>
				isSoftwareFullTheming
					? // eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- needed for theming
						Tokens.COLOR_BACKGROUND_NEUTRAL
					: // eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
						cardColor.default.mouseOver.backgroundColor,
			// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values, @atlaskit/ui-styling-standard/no-unsafe-values -- Ignored via go/DSP-18766
			color: cardColor.default.mouseOver.textColor,
		},
	},
	{
		'&:active': {
			// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
			backgroundColor: ({ isActive }) =>
				isActive
					? token('color.background.selected')
					: // eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
						cardColor.default.mouseDown.backgroundColor,
		},
	},
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	(props) =>
		props.isDragging && {
			'&, &:hover': {
				backgroundColor: token('color.background.selected'),
			},
		},
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	(props) =>
		props.isActive && {
			'&': {
				backgroundColor: props.isSoftwareFullTheming
					? // eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- needed for theming
						Tokens.COLOR_BACKGROUND_NEUTRAL
					: token('color.background.selected'),
				// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
				color: cardColor.active.mouseLeave.textColor,
				borderColor: props.isSoftwareFullTheming
					? // eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- needed for theming
						Tokens.COLOR_BORDER_SELECTED
					: token('color.border.selected'),
				borderStyle: 'solid',
				borderWidth: token('border.width'),
				borderRadius: token('radius.small'),
			},
			'&:hover, &:focus-within': {
				backgroundColor: props.isSoftwareFullTheming
					? // eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- needed for theming
						Tokens.COLOR_BACKGROUND_NEUTRAL_HOVERED
					: // eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
						cardColor.active.mouseOver.backgroundColor,
				// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
				color: cardColor.active.mouseOver.textColor,
			},
		},
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	(props) =>
		props.isDraggingOver && {
			'&, &:hover': {
				backgroundColor: token('color.background.selected'),
				color: token('color.text'),
			},
		},
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	(props) =>
		props.expanded &&
		!props.isActive && {
			backgroundColor: props.isSoftwareFullTheming
				? // eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- needed for theming
					Tokens.WRAPPER_BACKGROUND
				: token('elevation.surface'),
			borderColor: token('color.border'),
		},
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-unsafe-values -- Ignored via go/DSP-18766
	outlineStyles,
);

const InteractiveContainer = componentWithCondition(
	isVisualRefreshEnabled,
	InteractiveContainerNew,
	InteractiveContainerOld,
);

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled -- To migrate as part of go/ui-styling-standard
const Header = styled.div<{
	showChevron: boolean;
	children?: ReactNode;
	visualRefresh: boolean;
}>(
	{
		display: 'flex',
		alignItems: 'center',
		marginTop: token('space.025'),
		// eslint-disable-next-line @atlaskit/design-system/no-unsafe-design-token-usage -- The token value "4px" and fallback "3px" do not match and can't be replaced automatically.
		borderRadius: token('radius.small', '3px'),
		minHeight: token('space.300'),
	},
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-unsafe-values -- Ignored via go/DSP-18766
	outlineStyles,
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	({ showChevron, visualRefresh }) =>
		showChevron && !visualRefresh
			? { marginLeft: token('space.negative.100') }
			: { marginLeft: token('space.0') },
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles
	({ visualRefresh }) => visualRefresh && { minHeight: '28px' },
);

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled -- To migrate as part of go/ui-styling-standard
const Title = styled.div<{ visualRefresh: boolean }>(
	{
		display: 'inline-block',
		paddingTop: token('space.025'),
		wordWrap: 'break-word',
		overflowX: 'hidden',
		font: 'font.body',
		fontWeight: token('font.weight.medium'),
		alignSelf: 'center',
		flex: 1,
		// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values, @atlaskit/ui-styling-standard/no-unsafe-values -- Ignored via go/DSP-18766
		color: cardColor.default.textColor,
	},
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles
	({ visualRefresh }) => visualRefresh && { padding: token('space.025') },
);

const titleContainerStylesNew = xcss({
	font: 'font.body',
	fontWeight: token('font.weight.medium'),
	color: 'color.text.subtle',
	paddingLeft: 'space.0',
	margin: 'space.0',
	minHeight: 'space.300',
});

const titleContainerStyles = xcss({
	listStyle: 'none',
	color: 'color.text.subtle',
	paddingLeft: 'space.0',
	margin: 'space.0',
	minHeight: 'space.300',
});

const contentStyles = xcss({
	marginTop: 'space.100',
});

const titleSelectedStyles = xcss({
	color: 'color.text.selected',
});

const titleSelectedThemedStyles = xcss({
	// @ts-expect-error value does not match expected type
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values, @atlaskit/ui-styling-standard/no-unsafe-values -- needed for theming
	color: Tokens.COLOR_TEXT_SUBTLE,
});
