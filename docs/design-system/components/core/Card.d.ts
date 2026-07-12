import React from 'react';

/**
 * Card props.
 * @startingPoint section="Core" subtitle="Card — paper panel with optional grid watermark" viewport="700x260"
 */
export interface CardProps {
  children?: React.ReactNode;
  title?: React.ReactNode;
  /** Small uppercase label above the title. */
  eyebrow?: React.ReactNode;
  /** Content pinned to the header's right (e.g. a Badge). */
  right?: React.ReactNode;
  /** Faint graph-paper watermark behind content. */
  grid?: boolean;
  /** Selected/active — 2px graphite border. */
  selected?: boolean;
  elevation?: 0 | 1 | 2 | 3;
  padding?: string;
  onClick?: (e: React.MouseEvent<HTMLDivElement>) => void;
  style?: React.CSSProperties;
}

/** Paper surface container with hairline border and optional grid watermark. */
export function Card(props: CardProps): JSX.Element;
