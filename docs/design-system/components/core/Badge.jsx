import React from 'react';

/**
 * Graphite GP — Badge. Small status/label pill. Tones map to semantics:
 * ok (valid track), warn (run-out/caution), danger (crash), neutral, accent.
 */
export function Badge({ children, tone = 'neutral', solid = false, style = {}, ...rest }) {
  const tones = {
    neutral: { bg: 'var(--paper-2)', fg: 'var(--text-body)', bd: 'var(--border-hairline)', solidBg: 'var(--graphite-900)' },
    accent:  { bg: 'var(--accent-tint)', fg: 'var(--accent-press)', bd: 'var(--accent)', solidBg: 'var(--accent)' },
    ok:      { bg: 'var(--ok-tint)', fg: '#1E6B3C', bd: 'var(--ok)', solidBg: 'var(--ok)' },
    warn:    { bg: 'var(--warn-tint)', fg: '#8A6410', bd: 'var(--warn)', solidBg: 'var(--warn)' },
    danger:  { bg: 'var(--danger-tint)', fg: 'var(--danger)', bd: 'var(--danger)', solidBg: 'var(--danger)' },
  };
  const t = tones[tone] || tones.neutral;
  return (
    <span
      style={{
        display: 'inline-flex', alignItems: 'center', gap: '5px',
        height: '20px', padding: '0 8px',
        fontFamily: 'var(--font-mono)', fontSize: 'var(--fs-xs)', fontWeight: 'var(--fw-medium)',
        letterSpacing: 'var(--ls-mono)',
        color: solid ? 'var(--paper-0)' : t.fg,
        background: solid ? t.solidBg : t.bg,
        border: `var(--bw-hair) solid ${solid ? 'transparent' : t.bd}`,
        borderRadius: 'var(--radius-pill)',
        whiteSpace: 'nowrap',
        ...style,
      }}
      {...rest}
    >
      {children}
    </span>
  );
}
