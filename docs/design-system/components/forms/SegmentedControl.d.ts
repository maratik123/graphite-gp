import React from 'react';

export interface SegmentOption {
  value: string;
  label: React.ReactNode;
}

export interface SegmentedControlProps {
  /** Options as strings or { value, label } objects. */
  options: (string | SegmentOption)[];
  value: string;
  onChange?: (value: string) => void;
  size?: 'sm' | 'md' | 'lg';
  style?: React.CSSProperties;
}

/** Row of mutually exclusive options (difficulty, mode, shape). */
export function SegmentedControl(props: SegmentedControlProps): JSX.Element;
