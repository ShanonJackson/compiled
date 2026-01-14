/** @jsx jsx */
import React, { useState, useMemo, useCallback, useContext, createContext } from 'react';
import { css, jsx } from '@compiled/react';

const EditorThemeContext = createContext({ mode: 'light' });

const containerStyles = css({
  display: 'grid',
  gap: 16,
  padding: 24,
  borderRadius: 16,
  backgroundColor: '#F4F5F7',
});

const toolbarStyles = css({
  display: 'flex',
  flexWrap: 'wrap',
  gap: 8,
  button: {
    fontSize: 12,
    textTransform: 'uppercase',
    border: '1px solid transparent',
    borderRadius: 4,
    padding: '4px 8px',
    backgroundColor: 'transparent',
    cursor: 'pointer',
    ':hover': {
      borderColor: '#dfe1e6',
      backgroundColor: '#fff',
    },
    '&[data-active="true"]': {
      borderColor: '#7f5af0',
      color: '#7f5af0',
    },
  },
});

const editorStyles = css({
  minHeight: 200,
  padding: 16,
  backgroundColor: '#fff',
  borderRadius: 12,
  border: '1px solid #DFE1E6',
  fontSize: 15,
  lineHeight: 1.6,
  ':focus': {
    outline: '2px solid #4C9AFF',
    outlineOffset: 2,
  },
});

const statusStyles = css({
  display: 'flex',
  justifyContent: 'space-between',
  fontSize: 12,
  color: '#6B778C',
});

const badgeStyles = css({
  padding: '2px 6px',
  borderRadius: 999,
  border: '1px solid currentColor',
  fontSize: 10,
  textTransform: 'uppercase',
});

const ToneBadge = ({ mode }) => (
  <span
    css={badgeStyles}
    style={{ color: mode === 'dark' ? '#9BE7FF' : '#1D7AFC' }}>
    {mode} mode
  </span>
);

const ToolbarButton = ({ active, label, onClick }) => (
  <button type="button" data-active={active} onClick={onClick}>
    {label}
  </button>
);

const EditorShell = () => {
  const [value, setValue] = useState('Type something beautiful…');
  const [bold, setBold] = useState(false);
  const [italics, setItalics] = useState(false);
  const [mode, setMode] = useState('light');

  const wordCount = useMemo(() => value.trim().split(/\s+/).filter(Boolean).length, [value]);
  const handleToggle = useCallback((setter) => () => setter((prev) => !prev), []);
  const contextValue = useMemo(() => ({ mode, toggleMode: () => setMode((prev) => (prev === 'light' ? 'dark' : 'light')) }), [mode]);

  return (
    <EditorThemeContext.Provider value={contextValue}>
      <section css={containerStyles} aria-label="Rich text editor">
        <header css={toolbarStyles}>
          <ToolbarButton active={bold} label="Bold" onClick={handleToggle(setBold)} />
          <ToolbarButton active={italics} label="Italics" onClick={handleToggle(setItalics)} />
          <ToolbarButton
            active={mode === 'dark'}
            label="Toggle theme"
            onClick={() => contextValue.toggleMode()}
          />
        </header>
        <textarea
          css={editorStyles}
          style={{ fontWeight: bold ? 600 : 400, fontStyle: italics ? 'italic' : 'normal' }}
          value={value}
          onChange={(event) => setValue(event.target.value)}
        />
        <footer css={statusStyles}>
          <span>{wordCount} words</span>
          <EditorStatus />
        </footer>
      </section>
    </EditorThemeContext.Provider>
  );
};

const EditorStatus = () => {
  const { mode } = useContext(EditorThemeContext);
  return <ToneBadge mode={mode} />;
};

export const Component = () => <EditorShell />;
