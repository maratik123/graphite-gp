import React from 'react';

export interface IconButtonProps {
  /** A single icon glyph (Lucide <i data-lucide> or SVG). */
  children?: React.ReactNode;
  /** Accessible label (also the tooltip). Required for a11y. */
  label: string;
  variant?: 'secondary' | 'ghost';
  size?: 'sm' | 'md' | 'lg';
  /** Toggled/selected state — fills graphite. */
  active?: boolean;
  disabled?: boolean;
  onClick?: (e: React.MouseEvent<HTMLButtonElement>) => void;
  style?: React.CSSProperties;
}

/** Square, grid-aligned single-glyph button for toolbar / HUD actions. */
export function IconButton(props: IconButtonProps): JSX.Element;
