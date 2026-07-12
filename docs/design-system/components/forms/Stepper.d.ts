import React from 'react';

export interface StepperProps {
  value: number;
  min?: number;
  max?: number;
  step?: number;
  onChange?: (value: number) => void;
  label?: React.ReactNode;
  disabled?: boolean;
  style?: React.CSSProperties;
}

/** Integer +/- control for discrete counts (cars m, lap target, seed). */
export function Stepper(props: StepperProps): JSX.Element;
