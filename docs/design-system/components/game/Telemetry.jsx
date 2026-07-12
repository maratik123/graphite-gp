import React from 'react';

/**
 * Graphite GP — Telemetry. A mono-face readout of a single labelled metric
 * (speed, velocity vector, position, lap, tempo, Vmax). Compose several in a
 * row for a HUD strip. Tone colors the value for semantic emphasis.
 */
export function Telemetry({ label, value, unit = null, tone = 'default', size = 'md', align = 'left', style = {}, ...rest }) {
  const tones = {
    default: 'var(--text-ink)',
    accent: 'var(--accent)',
    ok: 'var(--ok)',
    warn: 'var(--warn)',
    danger: 'var(--danger)',
    muted: 'var(--text-muted)',
  };
  const valueSize = size === 'lg' ? 'var(--fs-h2)' : size === 'sm' ? 'var(--fs-title)' : 'var(--fs-h3)';
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 3, alignItems: align === 'right' ? 'flex-end' : 'flex-start', textAlign: align, ...style }} {...rest}>
      <span style={{
        fontFamily: 'var(--font-mono)', fontSize: 'var(--fs-xs)', textTransform: 'uppercase',
        letterSpacing: 'var(--ls-label)', color: 'var(--text-muted)', lineHeight: 1,
      }}>{label}</span>
      <span style={{
        fontFamily: 'var(--font-mono)', fontSize: valueSize, fontWeight: 'var(--fw-bold)',
        letterSpacing: 'var(--ls-mono)', color: tones[tone] || tones.default, lineHeight: 1,
        display: 'inline-flex', alignItems: 'baseline', gap: 4,
      }}>
        {value}
        {unit && <span style={{ fontSize: 'var(--fs-sm)', fontWeight: 'var(--fw-regular)', color: 'var(--text-muted)' }}>{unit}</span>}
      </span>
    </div>
  );
}
