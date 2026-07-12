import React from 'react';

/**
 * Graphite GP — MovePad. The signature control: the 5 von Neumann
 * accelerations laid out as a plus-shaped keypad — coast (·) in the center,
 * ↑ ↓ ← → around it. Each cell is one acceleration (a, b) ∈
 * {(0,0),(±1,0),(0,±1)}. `legal` masks illegal moves (grey, disabled);
 * `value` marks the chosen move.
 */
const MOVES = {
  up:    { a: 0,  b: 1,  arrow: '↑', grid: '1 / 2' },
  left:  { a: -1, b: 0,  arrow: '←', grid: '2 / 1' },
  coast: { a: 0,  b: 0,  arrow: '·', grid: '2 / 2' },
  right: { a: 1,  b: 0,  arrow: '→', grid: '2 / 3' },
  down:  { a: 0,  b: -1, arrow: '↓', grid: '3 / 2' },
};

export function MovePad({ value = null, legal = null, onSelect, size = 48, style = {}, ...rest }) {
  const cell = (key) => {
    const m = MOVES[key];
    const isLegal = legal ? legal.includes(key) : true;
    const selected = value === key;
    return (
      <button
        key={key} type="button" disabled={!isLegal}
        aria-label={`${key} (${m.a}, ${m.b})`}
        onClick={() => isLegal && onSelect && onSelect(key, { a: m.a, b: m.b })}
        style={{
          gridArea: m.grid, width: size, height: size,
          display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center',
          border: `var(--bw-1) solid ${selected ? 'var(--accent)' : isLegal ? 'var(--graphite-900)' : 'var(--border-soft)'}`,
          background: selected ? 'var(--accent)' : isLegal ? 'var(--paper-0)' : 'var(--paper-2)',
          color: selected ? 'var(--paper-0)' : isLegal ? 'var(--graphite-900)' : 'var(--text-faint)',
          borderRadius: 'var(--radius-0)', cursor: isLegal ? 'pointer' : 'not-allowed',
          fontFamily: 'var(--font-mono)', lineHeight: 1,
          transition: 'background var(--dur-fast) var(--ease-standard)',
        }}
      >
        <span style={{ fontSize: Math.round(size * 0.42), fontWeight: 700 }}>{m.arrow}</span>
        <span style={{ fontSize: Math.round(size * 0.19), opacity: 0.7, marginTop: 2 }}>{m.a},{m.b}</span>
      </button>
    );
  };
  return (
    <div
      style={{
        display: 'grid',
        gridTemplateColumns: `repeat(3, ${size}px)`,
        gridTemplateRows: `repeat(3, ${size}px)`,
        gap: 4, width: 'fit-content', ...style,
      }}
      {...rest}
    >
      {Object.keys(MOVES).map(cell)}
    </div>
  );
}
