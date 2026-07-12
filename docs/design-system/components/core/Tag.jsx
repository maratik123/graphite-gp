import React from 'react';

/**
 * Graphite GP — Tag. A grid-aligned (square) label chip, optionally removable
 * and with a leading color dot (e.g. a car color). More structural than Badge.
 */
export function Tag({ children, color = null, onRemove = null, selected = false, style = {}, ...rest }) {
  const [hover, setHover] = React.useState(false);
  return (
    <span
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        display: 'inline-flex', alignItems: 'center', gap: '7px',
        height: '26px', padding: '0 10px',
        fontFamily: 'var(--font-mono)', fontSize: 'var(--fs-sm)',
        color: 'var(--text-ink)',
        background: selected ? 'var(--paper-2)' : 'var(--paper-0)',
        border: `${selected ? 'var(--bw-1)' : 'var(--bw-hair)'} solid ${selected ? 'var(--border-strong)' : 'var(--border-hairline)'}`,
        borderRadius: 'var(--radius-0)',
        ...style,
      }}
      {...rest}
    >
      {color && (
        <span style={{ width: 10, height: 10, borderRadius: '50%', background: color, border: '1.5px solid var(--graphite-900)', flex: 'none' }} />
      )}
      {children}
      {onRemove && (
        <button
          type="button"
          onClick={onRemove}
          aria-label="Remove"
          style={{
            display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
            width: 16, height: 16, marginRight: -3,
            border: 'none', background: hover ? 'var(--paper-3)' : 'transparent',
            color: 'var(--text-muted)', cursor: 'pointer', borderRadius: 'var(--radius-1)',
            fontFamily: 'var(--font-mono)', fontSize: 13, lineHeight: 1, padding: 0,
          }}
        >×</button>
      )}
    </span>
  );
}
