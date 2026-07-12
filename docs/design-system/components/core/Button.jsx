import React from 'react';

/**
 * Graphite GP — Button.
 * Primary actions are GP-vermilion; secondary is a hairline paper button;
 * ghost is chromeless; danger signals a destructive/crash action.
 */
export function Button({
  children,
  variant = 'primary',
  size = 'md',
  disabled = false,
  iconLeft = null,
  iconRight = null,
  fullWidth = false,
  type = 'button',
  onClick,
  style = {},
  ...rest
}) {
  const [hover, setHover] = React.useState(false);
  const [active, setActive] = React.useState(false);

  const sizes = {
    sm: { h: 'var(--control-h-sm)', px: '12px', fs: 'var(--fs-sm)', gap: '6px' },
    md: { h: 'var(--control-h-md)', px: '16px', fs: 'var(--fs-body)', gap: '8px' },
    lg: { h: 'var(--control-h-lg)', px: '22px', fs: 'var(--fs-title)', gap: '10px' },
  };
  const s = sizes[size] || sizes.md;

  const palette = {
    primary: {
      bg: hover ? 'var(--accent-hover)' : 'var(--accent)',
      bgActive: 'var(--accent-press)',
      fg: 'var(--text-on-accent)',
      border: 'transparent',
    },
    secondary: {
      bg: hover ? 'var(--paper-2)' : 'var(--paper-0)',
      bgActive: 'var(--paper-3)',
      fg: 'var(--text-ink)',
      border: 'var(--border-strong)',
    },
    ghost: {
      bg: hover ? 'rgba(32,30,26,0.06)' : 'transparent',
      bgActive: 'rgba(32,30,26,0.12)',
      fg: 'var(--text-body)',
      border: 'transparent',
    },
    danger: {
      bg: hover ? 'var(--danger)' : 'var(--danger-tint)',
      bgActive: 'var(--accent-press)',
      fg: hover ? 'var(--text-on-accent)' : 'var(--danger)',
      border: 'var(--danger)',
    },
  };
  const p = palette[variant] || palette.primary;

  return (
    <button
      type={type}
      disabled={disabled}
      onClick={onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => { setHover(false); setActive(false); }}
      onMouseDown={() => setActive(true)}
      onMouseUp={() => setActive(false)}
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        justifyContent: 'center',
        gap: s.gap,
        height: s.h,
        padding: `0 ${s.px}`,
        width: fullWidth ? '100%' : 'auto',
        fontFamily: 'var(--font-ui)',
        fontSize: s.fs,
        fontWeight: 'var(--fw-semibold)',
        lineHeight: 1,
        color: p.fg,
        background: active && !disabled ? p.bgActive : p.bg,
        border: `var(--bw-1) solid ${p.border}`,
        borderRadius: 'var(--radius-2)',
        cursor: disabled ? 'not-allowed' : 'pointer',
        opacity: disabled ? 0.45 : 1,
        boxShadow: active && !disabled ? 'var(--shadow-inset)' : 'none',
        transform: active && !disabled ? 'translateY(1px)' : 'none',
        transition: 'background var(--dur-fast) var(--ease-standard), transform var(--dur-fast) var(--ease-standard)',
        whiteSpace: 'nowrap',
        ...style,
      }}
      {...rest}
    >
      {iconLeft}
      {children}
      {iconRight}
    </button>
  );
}
