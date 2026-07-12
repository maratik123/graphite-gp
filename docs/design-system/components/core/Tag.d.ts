import React from 'react';

export interface TagProps {
  children?: React.ReactNode;
  /** Optional leading color dot (e.g. a --car-* value). */
  color?: string | null;
  /** Show a remove (×) button; called on click. */
  onRemove?: (() => void) | null;
  selected?: boolean;
  style?: React.CSSProperties;
}

/** Grid-aligned label chip; supports a color dot and removability. */
export function Tag(props: TagProps): JSX.Element;
