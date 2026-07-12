import React from 'react';

export type MoveKey = 'up' | 'down' | 'left' | 'right' | 'coast';

/**
 * MovePad props.
 * @startingPoint section="Game" subtitle="MovePad — the 5-action accelerator" viewport="700x260"
 */
export interface MovePadProps {
  /** The currently chosen move key. */
  value?: MoveKey | null;
  /** Whitelist of legal move keys; others are greyed/disabled. null = all legal. */
  legal?: MoveKey[] | null;
  /** Called with (key, {a, b}) — the acceleration vector. */
  onSelect?: (key: MoveKey, accel: { a: number; b: number }) => void;
  /** Cell size in px. */
  size?: number;
  style?: React.CSSProperties;
}

/** The 5 von Neumann accelerations as a plus-shaped keypad — the signature Graphite GP control. */
export function MovePad(props: MovePadProps): JSX.Element;
