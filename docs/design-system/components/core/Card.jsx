import React from 'react';

/**
 * Graphite GP — Card. A paper surface with a hairline pencil border. Optional
 * faint graph-paper watermark, an eyebrow/title header, and a selected state
 * (2px graphite border). Prefer border structure over heavy shadow.
 */
export function Card({
  children,
  title = null,
  eyebrow = null,
  right = null,
  grid = false,
  selected = false,
  elevation = 1,
  padding = 'var(--space-5)',
  onClick,
  style = {},
  ...rest
}) {
  const shadows = { 0: 'var(--shadow-0)', 1: 'var(--shadow-1)', 2: 'var(--shadow-2)', 3: 'var(--shadow-3)' };
  return (
    <div
      onClick={onClick}
      style={{
        position: 'relative',
        background: 'var(--surface-card)',
        border: `${selected ? 'var(--bw-2)' : 'var(--bw-hair)'} solid ${selected ? 'var(--border-strong)' : 'var(--border-hairline)'}`,
        borderRadius: 'var(--radius-2)',
        boxShadow: shadows[elevation] ?? shadows[1],
        overflow: 'hidden',
        cursor: onClick ? 'pointer' : 'default',
        ...style,
      }}
      {...rest}
    >
      {grid && (
        <div style={{
          position: 'absolute', inset: 0, pointerEvents: 'none', opacity: 0.5,
          backgroundImage: 'var(--bg-grid)', backgroundSize: 'var(--cell) var(--cell)',
          backgroundAttachment: 'fixed',
        }} />
      )}
      <div style={{ position: 'relative', padding }}>
        {(eyebrow || title || right) && (
          <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', gap: 12, marginBottom: 'var(--space-3)' }}>
            <div>
              {eyebrow && (
                <div style={{
                  fontFamily: 'var(--font-mono)', fontSize: 'var(--fs-xs)', textTransform: 'uppercase',
                  letterSpacing: 'var(--ls-label)', color: 'var(--text-muted)', marginBottom: 4,
                }}>{eyebrow}</div>
              )}
              {title && (
                <div style={{
                  fontFamily: 'var(--font-display)', fontSize: 'var(--fs-title)', fontWeight: 'var(--fw-semibold)',
                  color: 'var(--text-ink)', lineHeight: 'var(--lh-snug)',
                }}>{title}</div>
              )}
            </div>
            {right && <div style={{ flex: 'none' }}>{right}</div>}
          </div>
        )}
        {children}
      </div>
    </div>
  );
}
