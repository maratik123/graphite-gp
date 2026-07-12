import React from 'react';

/**
 * Graphite GP — Slider. A grid-aligned range control. Used for continuous
 * parameters: pilot temperature, V_target, corridor width. Shows an optional
 * value readout in the mono face.
 */
export function Slider({
  value,
  min = 0,
  max = 100,
  step = 1,
  onChange,
  label = null,
  showValue = true,
  format = (v) => v,
  disabled = false,
  style = {},
  ...rest
}) {
  const pct = ((value - min) / (max - min)) * 100;
  return (
    <div style={{ width: '100%', opacity: disabled ? 0.5 : 1, ...style }}>
      {(label || showValue) && (
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'baseline', marginBottom: 8 }}>
          {label && (
            <span style={{
              fontFamily: 'var(--font-mono)', fontSize: 'var(--fs-xs)', textTransform: 'uppercase',
              letterSpacing: 'var(--ls-label)', color: 'var(--text-muted)',
            }}>{label}</span>
          )}
          {showValue && (
            <span style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--fs-sm)', fontWeight: 'var(--fw-medium)', color: 'var(--text-ink)' }}>
              {format(value)}
            </span>
          )}
        </div>
      )}
      <div style={{ position: 'relative', height: 20, display: 'flex', alignItems: 'center' }}>
        <div style={{ position: 'absolute', left: 0, right: 0, height: 4, background: 'var(--paper-3)', borderRadius: 'var(--radius-pill)' }} />
        <div style={{ position: 'absolute', left: 0, width: `${pct}%`, height: 4, background: 'var(--accent)', borderRadius: 'var(--radius-pill)' }} />
        <div style={{
          position: 'absolute', left: `calc(${pct}% - 9px)`, width: 18, height: 18,
          background: 'var(--paper-0)', border: 'var(--bw-2) solid var(--graphite-900)',
          borderRadius: '50%', boxShadow: 'var(--shadow-1)', pointerEvents: 'none',
        }} />
        <input
          type="range" value={value} min={min} max={max} step={step} disabled={disabled}
          onChange={(e) => onChange && onChange(Number(e.target.value))}
          style={{ position: 'absolute', left: 0, width: '100%', margin: 0, opacity: 0, height: 20, cursor: disabled ? 'not-allowed' : 'pointer' }}
          {...rest}
        />
      </div>
    </div>
  );
}
