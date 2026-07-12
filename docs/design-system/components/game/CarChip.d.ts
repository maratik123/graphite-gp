import React from 'react';

export interface CarChipProps {
  /** A --car-* color value. */
  color?: string;
  name: React.ReactNode;
  /** Standings position. */
  rank?: number | null;
  /** Marks the car as the player or an AI. */
  kind?: 'you' | 'ai' | null;
  /** Highlights the car whose turn it is. */
  active?: boolean;
  style?: React.CSSProperties;
}

/** Car token (color dot + name, optional rank / you-AI tag) for rosters & standings. */
export function CarChip(props: CarChipProps): JSX.Element;
