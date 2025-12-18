import React, {
	useMemo,
	useCallback,
	useState,
	useEffect,
	useRef,
	Component,
	type ElementRef,
	type ReactElement,
	type MouseEvent,
	memo,
} from 'react';
import { keyframes, styled } from '@compiled/react';
import intersection from 'lodash/intersection';
import isArray from 'lodash/isArray';
import join from 'lodash/join';
import round from 'lodash/round';
import { combine } from '@atlaskit/pragmatic-drag-and-drop/combine';
import { draggable } from '@atlaskit/pragmatic-drag-and-drop/element/adapter';
import { token } from '@atlaskit/tokens';
import {
	N300,
	B500,
	N500,
	N0,
	N1000,
	B700,
} from '@atlassian/jira-polaris-lib-color-palette/src/ui/colors/index.tsx';
import { fireUIAnalytics, useAnalyticsEvents } from '@atlassian/jira-product-analytics-bridge';
import { fg } from '@atlassian/jira-feature-gating';
import { DRAGGABLE_ITEM_TYPE } from '../../common/constants/index.tsx';
import { useMatrixActions } from '../../controllers/index.tsx';
import {
	useClusteredItemRendererComponent,
	useDraggableBubbleComponent,
	useItemRendererComponent,
} from '../../controllers/selectors/components-hooks.tsx';
import {
	useRelativeItemCenterPosition,
	useZAxisPositionForItems,
} from '../../controllers/selectors/positions-hooks.tsx';
import {
	useHoveredItems,
	useHighlightedItem,
	useSelectedItems,
} from '../../controllers/selectors/ui-hooks.tsx';
import {
	sendMatrixSingularDragOperationStarted,
	sendMatrixGroupDragOperationStarted,
	sendMatrixSingularDragOperationEnded,
	sendMatrixGroupDragOperationEnded,
	sendMatrixSingletonBubbleClicked,
	sendMatrixGroupBubbleClicked,
} from '../../services/pendo/index.tsx';
import type {
	ItemId,
	ItemProps,
	ClusteredItemProps,
	CenterPosition,
	ItemWrapperProps,
	DraggableBubbleProps,
} from '../../types.tsx';

type BubbleProps = {
	itemIds: ItemId[];
	isItemsDragDisabled?: boolean;
};

type BubblePositioningComponentState = {
	containerRef: ElementRef<'div'> | undefined;
};

type BubblePositioningComponentProps = {
	isHovered: boolean;
	isDragging: boolean;
	centerPosition: CenterPosition;
	zPosition: number;
	renderChildren: () => ReactElement;
};

// eslint-disable-next-line jira/react/no-class-components
class BubblePositioningComponent extends Component<
	BubblePositioningComponentProps,
	BubblePositioningComponentState
> {
	state = {
		containerRef: undefined,
	};

	shouldComponentUpdate(nextProps: BubblePositioningComponentProps) {
		return nextProps.renderChildren !== this.props.renderChildren;
	}

	componentDidUpdate(prevProps: Readonly<BubblePositioningComponentProps>): void {
		if (
			(this.props.zPosition !== prevProps.zPosition ||
				this.props.centerPosition !== prevProps.centerPosition) &&
			this.state.containerRef
		) {
			this.updateStylesForProps(this.state.containerRef, this.props);
		}
	}

	updateStylesForProps = (
		ref: ElementRef<'div'> | null,
		{ zPosition, centerPosition }: BubblePositioningComponentProps,
	) => {
		if (ref) {
			const { style } = ref;

			style.width = `${zPosition}px`;
			style.height = `${zPosition}px`;
			style.left = `calc(${centerPosition.left}% - ${zPosition / 2}px)`;
			style.top = `calc(${centerPosition.top}% - ${zPosition / 2}px)`;
			// bucketed z positions are 30-45-60-75, assigning z index with inverse proportion
			// will make sure that smaller bubbles are always on top of bigger bubbles,
			// hence all of the bubbles will be visible
			style.zIndex = Math.ceil((75 - zPosition) / 15 + 2).toString();
		}
	};

	setContainerRef = (containerRef: ElementRef<'div'>) => {
		this.setState({ containerRef });
		this.updateStylesForProps(containerRef, this.props);
	};

	render() {
		const { renderChildren, isHovered, isDragging } = this.props;

		return (
			<BubblePositioningContainer
				isDragging={isDragging}
				ref={this.setContainerRef}
				isHovered={isHovered}
			>
				{renderChildren()}
			</BubblePositioningContainer>
		);
	}
}

type DefaultBubbleComponentProps = {
	color: string;
	borderColor: string | string[];
} & ItemProps;

export const DefaultBubbleComponent = ({
	color,
	borderColor,
	isHovered,
	isSelected,
	isDragging,
	isHighlighted,
}: DefaultBubbleComponentProps) => (
	<BubbleContentWrapper
		data-testid="polaris-lib-matrix.ui.bubble.default-bubble-component-wrapper"
		data-component-selector="bubble-content-wrapper-phaV"
	>
		{!isDragging && (
			<BubbleAurora
				isSelected={isSelected}
				isHighlighted={isHighlighted}
				borderColor={borderColor}
				color={color}
				isHovered={isHovered}
			/>
		)}
		<DefaultBubbleContainer
			color={color}
			borderColor={borderColor}
			isHovered={isHovered}
			isSelected={isSelected}
			isHighlighted={isHighlighted}
			isDragging={isDragging}
		/>
	</BubbleContentWrapper>
);

type DefaultBubbleClusterComponentProps = {
	color: string;
	borderColor: string | string[];
} & ClusteredItemProps;

const getItemCountString = (items: string[]): string => {
	if (items.length < 1000) {
		return String(items.length);
	}

	return `${round(items.length / 1000, 1)}K`;
};

export const DefaultBubbleClusterComponent = ({
	items,
	color,
	borderColor,
	isHovered,
	isSelected,
	isDragging,
	isHighlighted,
}: DefaultBubbleClusterComponentProps) => (
	<BubbleContentWrapper
		data-testid="polaris-lib-matrix.ui.bubble.default-bubble-cluster-wrapper"
		data-component-selector="bubble-content-wrapper-phaV"
	>
		{!isDragging && (
			<BubbleAurora
				isSelected={isSelected}
				isHighlighted={isHighlighted}
				borderColor={borderColor}
				color={color}
				isHovered={isHovered}
			/>
		)}
		<DefaultBubbleContainer
			color={color}
			borderColor={borderColor}
			isHovered={isHovered}
			isSelected={isSelected}
			isHighlighted={isHighlighted}
			isDragging={isDragging}
		>
			<span>{getItemCountString(items)}</span>
		</DefaultBubbleContainer>
	</BubbleContentWrapper>
);

export const SimpleDefaultBubbleClusterComponent = ({
	items,
	isHovered,
	isSelected,
	isHighlighted,
}: ClusteredItemProps) => (
	<BubbleContentWrapper
		data-testid="polaris-lib-matrix.ui.bubble.simple-default-bubble-cluster-wrapper"
		data-component-selector="bubble-content-wrapper-phaV"
	>
		<BubbleAurora
			isSelected={isSelected}
			isHighlighted={isHighlighted}
			color={N300}
			borderColor={N300}
			isHovered={isHovered}
		/>
		<DefaultBubbleContainer
			color={N300}
			borderColor={N300}
			isHovered={isHovered}
			isSelected={isSelected}
			isHighlighted={isHighlighted}
		>
			<span>{getItemCountString(items)}</span>
		</DefaultBubbleContainer>
	</BubbleContentWrapper>
);

export const SimpleDefaultBubbleComponent = ({
	isHovered,
	isSelected,
	isHighlighted,
}: ItemProps) => (
	<BubbleContentWrapper
		data-testid="polaris-lib-matrix.ui.bubble.simple-default-bubble-wrapper"
		data-component-selector="bubble-content-wrapper-phaV"
	>
		<BubbleAurora
			isSelected={isSelected}
			isHighlighted={isHighlighted}
			color={N300}
			borderColor={N300}
			isHovered={isHovered}
		/>
		<DefaultBubbleContainer
			color={N300}
			borderColor={N300}
			isHovered={isHovered}
			isSelected={isSelected}
			isHighlighted={isHighlighted}
		/>
	</BubbleContentWrapper>
);

export const SimpleDefaultBubbleWrapperComponent = (props: ItemWrapperProps) => (
	<>{props.children}</>
);

export const DraggableBubble = ({
	children,
	id,
	itemIds,
	onDragStart,
	onDragEnd,
	canDrop = true,
}: DraggableBubbleProps) => {
	const { top, left } = useRelativeItemCenterPosition(id);
	const ref = useRef<HTMLDivElement>(null);

	useEffect(() => {
		if (!ref.current) return undefined;

		const cleanupDragAndDrop = combine(
			draggable({
				element: ref.current,
				getInitialData() {
					return { id, itemIds, type: DRAGGABLE_ITEM_TYPE, top, left, canDrop };
				},
				onDragStart() {
					if (itemIds.length > 1) {
						sendMatrixGroupDragOperationStarted();
					} else {
						sendMatrixSingularDragOperationStarted();
					}

					onDragStart();
				},
				onDrop() {
					if (itemIds.length > 1) {
						sendMatrixGroupDragOperationEnded();
					} else {
						sendMatrixSingularDragOperationEnded();
					}
					onDragEnd();
				},
			}),
		);

		return () => {
			cleanupDragAndDrop?.();
		};
	}, [canDrop, id, itemIds, left, onDragEnd, onDragStart, top]);

	return (
		<DraggableBubbleWrapper
			data-testid="polaris-lib-matrix.ui.bubble.draggable-bubble-wrapper"
			ref={ref}
		>
			{children}
		</DraggableBubbleWrapper>
	);
};

export const Bubble = memo(({ itemIds, isItemsDragDisabled }: BubbleProps) => {
	const positioningId = itemIds[0];
	const [isDragging, setIsDragging] = useState(false);
	const toggleIsDragging = useCallback(() => setIsDragging((prev) => !prev), []);

	const BubbleRenderComponent = useItemRendererComponent();
	const BubbleClusterRenderComponent = useClusteredItemRendererComponent();
	const DraggableBubbleComponentProvided = useDraggableBubbleComponent();

	const InnerBubbleComponent = BubbleRenderComponent || SimpleDefaultBubbleComponent;
	const InnerBubbleClusterComponent =
		BubbleClusterRenderComponent || SimpleDefaultBubbleClusterComponent;
	const DraggableBubbleComponent = fg('jpd-aurora-roadmap-inline-edit')
		? DraggableBubbleComponentProvided || DraggableBubble
		: DraggableBubble;

	const centerPosition = useRelativeItemCenterPosition(positioningId);
	const zPosition = useZAxisPositionForItems(itemIds);
	const highlightedItem = useHighlightedItem();
	const hoveredItem = useHoveredItems();
	const selectedItems = useSelectedItems();
	const { createAnalyticsEvent } = useAnalyticsEvents();

	const [, actions] = useMatrixActions();
	const { setHoveredItems, setSelectedItems } = actions;

	const isHovered = useMemo(
		() => intersection(itemIds, hoveredItem?.items || []).length > 0,
		[hoveredItem?.items, itemIds],
	);

	const isSelected = useMemo(
		() => intersection(selectedItems || [], itemIds || []).length > 0,
		[itemIds, selectedItems],
	);

	const isHighlighted = useMemo(
		() => !!highlightedItem && itemIds.includes(highlightedItem),
		[itemIds, highlightedItem],
	);

	const setSelectionFromClickEvent = useCallback(
		(e: MouseEvent<HTMLElement>) => {
			// do not propagate click event upwards to enable "outside-of-bubble-click" handling on the container
			e.stopPropagation();
			const analyticsAttributes = {
				bubbleType: 'single',
				multiSelect: e.shiftKey || e.metaKey || e.ctrlKey,
			};
			if (itemIds.length > 1) {
				sendMatrixGroupBubbleClicked();
				analyticsAttributes.bubbleType = 'clustered';
			} else {
				sendMatrixSingletonBubbleClicked();
			}

			fireUIAnalytics(
				createAnalyticsEvent({ action: 'clicked', actionSubject: 'icon' }),
				'ideaBubble',
				analyticsAttributes,
			);

			const selected = selectedItems ?? [];

			if (selected.length === 0) {
				if (e.shiftKey) return;
				setSelectedItems([...itemIds]);
			} else if (e.metaKey || e.ctrlKey) {
				// in case current bubble is already selected, filter out current ones first
				const otherSelected = [...selected.filter((id) => !itemIds.includes(id))];
				setSelectedItems([...otherSelected, ...itemIds]);
			} else if (e.shiftKey) {
				setSelectedItems([...selected.filter((id) => !itemIds.includes(id))]);
			} else {
				setSelectedItems([...itemIds]);
			}
		},
		[setSelectedItems, selectedItems, itemIds, createAnalyticsEvent],
	);

	const setHover = useCallback(() => {
		setHoveredItems({ items: itemIds, area: 'MATRIX' });
	}, [itemIds, setHoveredItems]);

	const clearHover = useCallback(() => {
		setHoveredItems(undefined);
	}, [setHoveredItems]);

	const renderInnerBubble = useCallback(
		() => {
			const InnerBubble =
				itemIds.length > 1 ? (
					<InnerBubbleClusterComponent
						items={itemIds}
						isHovered={isHovered}
						isSelected={isSelected}
						isDragging={isDragging}
						isHighlighted={isHighlighted}
					/>
				) : (
					<InnerBubbleComponent
						id={positioningId}
						isHovered={isHovered}
						isSelected={isSelected}
						isDragging={isDragging}
						isHighlighted={isHighlighted}
					/>
				);

			return isItemsDragDisabled ? (
				InnerBubble
			) : (
				<DraggableBubbleComponent
					id={positioningId}
					itemIds={itemIds}
					onDragStart={toggleIsDragging}
					onDragEnd={toggleIsDragging}
				>
					{InnerBubble}
				</DraggableBubbleComponent>
			);
		},
		// eslint-disable-next-line react-hooks/react-compiler
		// eslint-disable-next-line react-hooks/exhaustive-deps
		[itemIds, positioningId, isSelected, isHovered, isDragging, isHighlighted],
	);

	return (
		// eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions, @atlassian/a11y/interactive-element-not-keyboard-focusable
		<div onMouseEnter={setHover} onMouseLeave={clearHover} onClick={setSelectionFromClickEvent}>
			<BubblePositioningComponent
				centerPosition={centerPosition}
				zPosition={zPosition}
				isHovered={isHovered}
				isDragging={isDragging}
				renderChildren={renderInnerBubble}
			/>
		</div>
	);
});

const pulseAnimation = keyframes({
	'0%': {
		transform: 'scale(1)',
		opacity: 0.5,
	},
	'50%': {
		left: '-8px',
		top: '-8px',
		height: 'calc(100% + 16px)',
		width: 'calc(100% + 16px)',
		opacity: 0.5,
	},
	'100%': {
		transform: 'scale(1)',
		opacity: 0.5,
	},
});

const getConicGradient = (borderColor: string[]): string => {
	const segmentAngle = 360 / borderColor.length;

	// 0deg is at top center position, we want it at left center
	const angleShift = -90;
	const gradientSegments: string[] = [];
	const overflowSegments: string[] = [];

	borderColor.forEach((segmentColor, index) => {
		const startAngle = index * segmentAngle + angleShift;
		const endAngle = (index + 1) * segmentAngle + angleShift;

		if (startAngle >= 0) {
			gradientSegments.push(`${segmentColor} ${startAngle}deg ${endAngle}deg`);
		} else {
			overflowSegments.push(`${segmentColor} ${360 + startAngle}deg ${360}deg`);
			gradientSegments.push(`${segmentColor} ${0}deg ${endAngle}deg`);
		}
	});

	return join([...gradientSegments, ...overflowSegments], ', ');
};

const getBorderColor = (isHovered: boolean, borderColor: string | string[]) => {
	if (isArray(borderColor) && borderColor.length > 1) {
		return 'transparent';
	}
	let bc = borderColor;
	if (isArray(borderColor)) {
		[bc] = borderColor;
	} else if (isHovered) {
		bc = N500;
	}
	return bc;
};

const BORDER_THICKNESS = 3;

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled -- To migrate as part of go/ui-styling-standard
const BubbleContentWrapper = styled.div({
	boxSizing: 'border-box',
	borderRadius: token('radius.full'),
	position: 'relative',
	backgroundColor: 'transparent',
	width: '100%',
	height: '100%',
});

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled -- To migrate as part of go/ui-styling-standard
const BubbleAurora = styled.div<{
	color: string;
	borderColor: string | string[];
	isSelected: boolean;
	isHighlighted: boolean;
	isHovered: boolean;
}>({
	boxSizing: 'border-box',
	borderRadius: token('radius.full'),
	position: 'absolute',
	left: 0,
	top: 0,
	width: '100%',
	height: '100%',
	// background
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	backgroundColor: ({ isHighlighted, isSelected }) =>
		// eslint-disable-next-line no-nested-ternary, @atlaskit/ui-styling-standard/no-imported-style-values
		isHighlighted ? B700 : isSelected ? B500 : 'initial',
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	backgroundImage: ({ isHighlighted, isSelected, borderColor }) =>
		!isSelected && !isHighlighted
			? `conic-gradient(${getConicGradient(
					// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
					isArray(borderColor) && borderColor.length > 0 ? borderColor : [N500],
				)})`
			: 'initial',
	backgroundSize: '100% 100%',
	backgroundRepeat: 'no-repeat',
	backgroundPosition: 'center center, top left',
	backgroundOrigin: 'content-box, border-box',
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	backgroundClip: ({ isSelected }) => !isSelected && 'content-box, border-box',
	// animation
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-nested-selectors, @atlaskit/ui-styling-standard/no-unsafe-selectors -- Ignored via go/DSP-18766
	'[data-component-selector="bubble-content-wrapper-phaV"]:hover &': {
		animationName: pulseAnimation,
		animationDuration: '2s',
		animationIterationCount: 'infinite',
	},
});

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled -- To migrate as part of go/ui-styling-standard
const DefaultBubbleContainer = styled.div<{
	borderColor: string | string[];
	color: string;
	isHovered: boolean;
	isDragging?: boolean;
	isSelected: boolean;
	isHighlighted: boolean;
}>({
	width: '100%',
	height: '100%',
	position: 'absolute',
	left: 0,
	top: 0,
	boxSizing: 'border-box',
	borderRadius: token('radius.full'),
	display: 'flex',
	alignItems: 'center',
	justifyContent: 'center',
	font: token('font.body.UNSAFE_small'),
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	opacity: ({ isDragging }) => (isDragging ? '0' : '1'),
	transition: '0.5s',
	'&:hover': {
		cursor: 'pointer',
	},
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-nested-selectors -- Ignored via go/DSP-18766
	'& > span': {
		userSelect: 'none',
	},
	// border
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	border: ({ isHighlighted, isHovered, borderColor, isSelected }) =>
		!isHighlighted &&
		!isSelected &&
		`${BORDER_THICKNESS}px solid ${getBorderColor(isHovered, borderColor)}`,
	// background
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	backgroundImage: ({ isHighlighted, borderColor, color, isHovered, isSelected }) =>
		!isSelected &&
		// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
		isArray(borderColor) &&
		borderColor.length > 0 &&
		// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values, no-nested-ternary -- Ignored via go/DSP-18766
		`linear-gradient(${isHighlighted ? B700 : isSelected ? B500 : isHovered ? N500 : color}, ${
			// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values, no-nested-ternary -- Ignored via go/DSP-18766
			isHighlighted ? B700 : isSelected ? B500 : isHovered ? N500 : color
		}), conic-gradient(${getConicGradient(borderColor)})`,
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	backgroundColor: ({ isHighlighted, isSelected, color }) =>
		// eslint-disable-next-line no-nested-ternary, @atlaskit/ui-styling-standard/no-imported-style-values
		isHighlighted ? B700 : isSelected ? B500 : color,
	backgroundSize: '100% 100%, 100% 100%',
	backgroundRepeat: 'no-repeat',
	backgroundPosition: 'center center, top left',
	backgroundOrigin: 'content-box, border-box',
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	backgroundClip: ({ borderColor }) =>
		// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
		isArray(borderColor) && borderColor.length > 0 && 'content-box, border-box',
	// bubble color
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	color: ({ isHighlighted, isHovered, isSelected }) =>
		// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values
		isHighlighted || isSelected || isHovered ? N0 : N1000,
	// hover state
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-nested-selectors, @atlaskit/ui-styling-standard/no-unsafe-selectors -- Ignored via go/DSP-18766
	'[data-component-selector="bubble-content-wrapper-phaV"]:hover &': {
		// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
		backgroundColor: ({ isHighlighted, isSelected, borderColor }) =>
			// eslint-disable-next-line no-nested-ternary -- Ignored via go/DSP-18766
			isHighlighted
				? // eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values
					B700
				: // eslint-disable-next-line no-nested-ternary
					isSelected
					? // eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values
						B500
					: // eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values
						isArray(borderColor) && borderColor.length === 1
						? borderColor[0]
						: // eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values
							N500,
		// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
		backgroundImage: ({ borderColor, isSelected }) =>
			!isSelected &&
			// eslint-disable-next-line @atlaskit/ui-styling-standard/no-imported-style-values -- Ignored via go/DSP-18766
			isArray(borderColor) &&
			borderColor.length === 1 &&
			`linear-gradient(${borderColor[0]}, ${
				borderColor[0]
			}), conic-gradient(${getConicGradient(borderColor)})`,
	},
});

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled -- To migrate as part of go/ui-styling-standard
const DraggableBubbleWrapper = styled.div({
	height: 'inherit',
});

// eslint-disable-next-line @atlaskit/ui-styling-standard/no-styled -- To migrate as part of go/ui-styling-standard
const BubblePositioningContainer = styled.div<{
	isDragging: boolean;
	isHovered: boolean;
}>({
	position: 'absolute',
	boxSizing: 'border-box',
	borderRadius: token('radius.full'),
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	transition: ({ isDragging }) => (isDragging ? 'initial' : '0.5s ease-in-out'),
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-dynamic-styles -- Ignored via go/DSP-18766
	zIndex: ({ isHovered }) => (isHovered ? 3 : 2),
	/*
	workaround for tooltip container problem,
	without this the bubbles become ellipsis
	*/
	// eslint-disable-next-line @atlaskit/ui-styling-standard/no-nested-selectors -- Ignored via go/DSP-18766
	'& div[role="presentation"]': {
		height: 'inherit',
	},
});
