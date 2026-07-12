import React from 'react';

/**
 * Graphite GP — LapMeter. Lap progress as a row of cells that fill as laps
 * complete, plus a mono "n/total" readout. The signed lap counter drives it.
 */
export function LapMeter({ lap = 0, total = 5, label = 'LAP', style = {}, ...rest }) {
  const done = Math.max(0, Math.min(total, lap));
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 6, ...style }} {...rest}>
      <div style={{ display: 'flex', alignItems: 'baseline', justifyContent: 'space-between', gap: 12 }}>
        <span style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--fs-xs)', textTransform: 'uppercase', letterSpacing: 'var(--ls-label)', color: 'var(--text-muted)' }}>{label}</span>
        <span style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--fs-title)', fontWeight: 'var(--fw-bold)', color: 'var(--text-ink)', lineHeight: 1 }}>
          {done}<span style={{ color: 'var(--text-faint)' }}>/{total}</span>
        </span>
      </div>
      <div style={{ display: 'flex', gap: 3 }}>
        {Array.from({ length: total }).map((_, i) => (
          <span key={i} style={{
            flex: 1, height: 8,
            background: i < done ? 'var(--accent)' : 'var(--paper-3)',
            border: 'var(--bw-hair) solid var(--graphite-900)',
            borderRadius: 'var(--radius-0)',
          }} />
        ))}
      </div>
    </div>
  );
}
