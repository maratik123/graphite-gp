import React from 'react';

export interface SliderProps {
  value: number;
  min?: number;
  max?: number;
  step?: number;
  onChange?: (value: number) => void;
  /** Uppercase mono label above the track. */
  label?: React.ReactNode;
  showValue?: boolean;
  /** Format the value readout. */
  format?: (v: number) => React.ReactNode;
  disabled?: boolean;
  style?: React.CSSProperties;
}

/** Range control for continuous params (temperature, V_target, width). */
export function Slider(props: SliderProps): JSX.Element;
