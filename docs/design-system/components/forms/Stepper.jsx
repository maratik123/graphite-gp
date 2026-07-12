import React from 'react';

/**
 * Graphite GP — Stepper. Integer +/- control on the lattice. Used for discrete
 * counts: number of cars (m), lap target, seed. Value shown in the mono face.
 */
export function Stepper({ value, min = 0, max = 99, step = 1, onChange, label = null, disabled = false, style = {}, ...rest }) {
  const set = (v) => {
    const clamped = Math.max(min, Math.min(max, v));
    onChange && onChange(clamped);
  };
  const btn = (content, fn, dis) => (
    <button
      type="button" onClick={fn} disabled={dis} aria-hidden={false}
      style={{
        width: 34, height: 34, flex: 'none', display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
        border: 'none', background: 'transparent', color: dis ? 'var(--text-faint)' : 'var(--text-ink)',
        fontFamily: 'var(--font-mono)', fontSize: 18, lineHeight: 1, cursor: dis ? 'not-allowed' : 'pointer',
      }}
    >{content}</button>
  );
  return (
    <div style={{ display: 'inline-flex', flexDirection: 'column', gap: 8, ...style }}>
      {label && (
        <span style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--fs-xs)', textTransform: 'uppercase', letterSpacing: 'var(--ls-label)', color: 'var(--text-muted)' }}>{label}</span>
      )}
      <div style={{
        display: 'inline-flex', alignItems: 'center', height: 'var(--control-h-md)',
        border: 'var(--bw-1) solid var(--graphite-900)', borderRadius: 'var(--radius-2)',
        background: 'var(--paper-0)', opacity: disabled ? 0.5 : 1, overflow: 'hidden', width: 'fit-content',
      }}>
        {btn('−', () => set(value - step), disabled || value <= min)}
        <span style={{
          minWidth: 40, textAlign: 'center', fontFamily: 'var(--font-mono)', fontSize: 'var(--fs-title)',
          fontWeight: 'var(--fw-medium)', color: 'var(--text-ink)',
          borderLeft: 'var(--bw-hair) solid var(--border-hairline)', borderRight: 'var(--bw-hair) solid var(--border-hairline)',
          padding: '0 4px', lineHeight: 'var(--control-h-md)',
        }}>{value}</span>
        {btn('+', () => set(value + step), disabled || value >= max)}
      </div>
    </div>
  );
}
