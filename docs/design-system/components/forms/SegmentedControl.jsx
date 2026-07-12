import React from 'react';

/**
 * Graphite GP — SegmentedControl. A grid-aligned row of mutually exclusive
 * options (difficulty, overlay mode, track shape). The selected segment fills
 * graphite; options sit flush like cells in a row.
 */
export function SegmentedControl({ options = [], value, onChange, size = 'md', style = {}, ...rest }) {
  const h = size === 'sm' ? 'var(--control-h-sm)' : size === 'lg' ? 'var(--control-h-lg)' : 'var(--control-h-md)';
  const fs = size === 'sm' ? 'var(--fs-sm)' : 'var(--fs-body)';
  return (
    <div
      role="tablist"
      style={{
        display: 'inline-flex', height: h,
        border: 'var(--bw-1) solid var(--graphite-900)', borderRadius: 'var(--radius-2)',
        overflow: 'hidden', background: 'var(--paper-0)', ...style,
      }}
      {...rest}
    >
      {options.map((opt, i) => {
        const val = typeof opt === 'object' ? opt.value : opt;
        const lbl = typeof opt === 'object' ? opt.label : opt;
        const sel = val === value;
        return (
          <button
            key={val} type="button" role="tab" aria-selected={sel}
            onClick={() => onChange && onChange(val)}
            style={{
              display: 'inline-flex', alignItems: 'center', justifyContent: 'center', gap: 6,
              padding: '0 14px', height: '100%',
              border: 'none', borderLeft: i === 0 ? 'none' : 'var(--bw-hair) solid var(--graphite-900)',
              background: sel ? 'var(--graphite-900)' : 'transparent',
              color: sel ? 'var(--paper-0)' : 'var(--text-body)',
              fontFamily: 'var(--font-ui)', fontSize: fs, fontWeight: 'var(--fw-medium)',
              cursor: 'pointer', whiteSpace: 'nowrap',
              transition: 'background var(--dur-fast) var(--ease-standard)',
            }}
          >{lbl}</button>
        );
      })}
    </div>
  );
}
