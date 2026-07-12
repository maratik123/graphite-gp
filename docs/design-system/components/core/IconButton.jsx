import React from 'react';

/**
 * Graphite GP — IconButton. A square, grid-aligned button for a single glyph
 * (Lucide). Use for toolbar/HUD actions: play, pause, settings, zoom, overlays.
 */
export function IconButton({
  children,
  label,
  variant = 'secondary',
  size = 'md',
  active = false,
  disabled = false,
  onClick,
  style = {},
  ...rest
}) {
  const [hover, setHover] = React.useState(false);
  const [press, setPress] = React.useState(false);

  const sizes = { sm: 30, md: 38, lg: 46 };
  const dim = sizes[size] || sizes.md;

  const bg = active
    ? 'var(--graphite-900)'
    : variant === 'ghost'
      ? (hover ? 'rgba(32,30,26,0.06)' : 'transparent')
      : (hover ? 'var(--paper-2)' : 'var(--paper-0)');
  const fg = active ? 'var(--paper-0)' : 'var(--text-ink)';
  const border = active
    ? 'var(--graphite-900)'
    : variant === 'ghost' ? 'transparent' : 'var(--border-strong)';

  const pressBg = active
    ? 'var(--graphite-900)'
    : variant === 'ghost' ? 'rgba(32,30,26,0.12)' : 'var(--paper-3)';

  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => { setHover(false); setPress(false); }}
      onMouseDown={() => setPress(true)}
      onMouseUp={() => setPress(false)}
      style={{
        width: dim, height: dim,
        display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
        color: fg,
        background: press && !disabled ? pressBg : bg,
        border: `var(--bw-1) solid ${border}`,
        borderRadius: 'var(--radius-2)',
        cursor: disabled ? 'not-allowed' : 'pointer',
        opacity: disabled ? 0.45 : 1,
        boxShadow: press && !disabled ? 'var(--shadow-inset)' : 'none',
        transition: 'background var(--dur-fast) var(--ease-standard)',
        ...style,
      }}
      {...rest}
    >
      {children}
    </button>
  );
}
