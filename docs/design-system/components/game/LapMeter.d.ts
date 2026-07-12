import React from 'react';

export interface LapMeterProps {
  /** Laps completed (from the signed lap counter, clamped ≥ 0). */
  lap?: number;
  total?: number;
  label?: React.ReactNode;
  style?: React.CSSProperties;
}

/** Lap progress: cells that fill as laps close + a mono n/total readout. */
export function LapMeter(props: LapMeterProps): JSX.Element;
