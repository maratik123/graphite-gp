import React from 'react';

/**
 * Graphite GP — Switch. A toggle for boolean overlays/options (speed heatmap,
 * fastest-lap line, grid). Reads like a pressed graphite key when on.
 * The whole control (track + label) is clickable.
 */
export function Switch({ checked = false, onChange, label = null, disabled = false, style = {}, ...rest }) {
  const track = checked ? 'var(--accent)' : 'var(--paper-3)';
  const toggle = () => { if (!disabled && onChange) onChange(!checked); };
  const onKey = (e) => {
    if (disabled) return;
    if (e.key === ' ' || e.key === 'Enter') { e.preventDefault(); toggle(); }
  };
  return (
    <div
      role="switch" aria-checked={checked} aria-disabled={disabled || undefined}
      tabIndex={disabled ? -1 : 0}
      onClick={toggle} onKeyDown={onKey}
      style={{
        display: 'inline-flex', alignItems: 'center', gap: 10,
        cursor: disabled ? 'not-allowed' : 'pointer', opacity: disabled ? 0.5 : 1,
        userSelect: 'none', ...style,
      }}
      {...rest}
    >
      <span style={{
        position: 'relative', width: 40, height: 22, flex: 'none',
        background: track, border: 'var(--bw-1) solid var(--graphite-900)',
        borderRadius: 'var(--radius-pill)', transition: 'background var(--dur-fast) var(--ease-standard)',
      }}>
        <span style={{
          position: 'absolute', top: 2, left: checked ? 20 : 2, width: 16, height: 16,
          background: 'var(--paper-0)', border: '1.5px solid var(--graphite-900)', borderRadius: '50%',
          transition: 'left var(--dur-fast) var(--ease-standard)',
        }} />
      </span>
      {label && (
        <span style={{ fontFamily: 'var(--font-ui)', fontSize: 'var(--fs-body)', color: 'var(--text-body)' }}>{label}</span>
      )}
    </div>
  );
}
