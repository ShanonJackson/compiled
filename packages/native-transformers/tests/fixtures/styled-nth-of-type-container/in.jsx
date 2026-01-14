import { styled } from '@compiled/react';

const ROW_GAP = 4;
const COLUMN_GAP = 8;
const FIELD_HEIGHT = 30;
const FIELD_HEIGHT_COMPACT = 20;
const DISPLAYING_FIELDS_MIN_CARD_WIDTH = 75;

// A reduced version of the idea-card FieldsContainer styles that produced a selector hash mismatch
// between Babel and SWC. Keep the nth-of-type selector and the container query intact.
const FieldsContainer = styled.div({
  display: ({ isRow }) => (isRow ? 'flex' : 'grid'),
  gap: ({ isRow }) => (isRow ? `0 ${COLUMN_GAP}px` : `${ROW_GAP}px ${COLUMN_GAP}px`),
  alignItems: 'center',
  width: ({ isRow, hasMaxContent }) => (isRow && hasMaxContent ? 'max-content' : '100%'),
  gridTemplateColumns: ({ isRow }) => !isRow && '[title] fit-content(100px) [content] 1fr',
  gridAutoRows: ({ isRow }) => !isRow && `minmax(${FIELD_HEIGHT}px, auto)`,
  height: ({ isRow }) => isRow && `${FIELD_HEIGHT_COMPACT + ROW_GAP}px`,
  '&:empty': {
    display: 'none',
  },
  '& > div:nth-of-type(1n+4)': {
    display: ({ isRow, cappedFieldsDisplay }) =>
      isRow && cappedFieldsDisplay ? 'none' : undefined,
  },
  [`@container cardContainer (max-width: ${DISPLAYING_FIELDS_MIN_CARD_WIDTH}px)`]: {
    width: 0,
    overflowX: 'hidden',
    overflowY: 'hidden',
    visibility: 'hidden',
  },
});

export const Component = () => (
  <FieldsContainer isRow hasMaxContent cappedFieldsDisplay>
    <div />
    <div />
    <div />
    <div />
  </FieldsContainer>
);
