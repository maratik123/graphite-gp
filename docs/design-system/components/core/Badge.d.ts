import React from 'react';

export interface BadgeProps {
  children?: React.ReactNode;
  /** Semantic tone. */
  tone?: 'neutral' | 'accent' | 'ok' | 'warn' | 'danger';
  /** Filled (solid) vs tinted. */
  solid?: boolean;
  style?: React.CSSProperties;
}

/** Small status/label pill in the mono face. */
export function Badge(props: BadgeProps): JSX.Element;
