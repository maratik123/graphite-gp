import React from 'react';

/**
 * Graphite GP — CarChip. A car token: colored dot + label, optional rank and
 * "you"/AI tag. Used in standings, rosters, and legends. `active` highlights
 * the car whose turn it is.
 */
export function CarChip({ color = 'var(--car-1)', name, rank = null, kind = null, active = false, style = {}, ...rest }) {
  return (
    <div
      style={{
        display: 'inline-flex', alignItems: 'center', gap: 10,
        height: 34, padding: '0 12px 0 8px',
        background: active ? 'var(--paper-2)' : 'var(--paper-0)',
        border: `${active ? 'var(--bw-2)' : 'var(--bw-hair)'} solid ${active ? 'var(--graphite-900)' : 'var(--border-hairline)'}`,
        borderRadius: 'var(--radius-1)', ...style,
      }}
      {...rest}
    >
      {rank != null && (
        <span style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--fs-title)', fontWeight: 'var(--fw-bold)', color: 'var(--text-ink)', minWidth: 18, textAlign: 'center' }}>{rank}</span>
      )}
      <span style={{ width: 16, height: 16, borderRadius: '50%', background: color, border: '2px solid var(--graphite-900)', flex: 'none' }} />
      <span style={{ fontFamily: 'var(--font-ui)', fontSize: 'var(--fs-body)', fontWeight: 'var(--fw-medium)', color: 'var(--text-ink)' }}>{name}</span>
      {kind && (
        <span style={{
          fontFamily: 'var(--font-mono)', fontSize: 'var(--fs-micro)', textTransform: 'uppercase', letterSpacing: 'var(--ls-label)',
          color: kind === 'you' ? 'var(--accent)' : 'var(--text-muted)',
          border: `var(--bw-hair) solid ${kind === 'you' ? 'var(--accent)' : 'var(--border-hairline)'}`,
          borderRadius: 'var(--radius-pill)', padding: '1px 6px',
        }}>{kind === 'you' ? 'YOU' : 'AI'}</span>
      )}
    </div>
  );
}
