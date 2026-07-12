import React from 'react';

export interface SwitchProps {
  checked?: boolean;
  onChange?: (checked: boolean) => void;
  label?: React.ReactNode;
  disabled?: boolean;
  style?: React.CSSProperties;
}

/** Boolean toggle for overlays/options (heatmap, fastest-lap, grid). */
export function Switch(props: SwitchProps): JSX.Element;
