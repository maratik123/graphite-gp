import React from 'react';

export interface TelemetryProps {
  /** Uppercase field name, e.g. "SPEED", "LAP". */
  label: React.ReactNode;
  /** The value — usually a mono number/vector. */
  value: React.ReactNode;
  unit?: React.ReactNode;
  tone?: 'default' | 'accent' | 'ok' | 'warn' | 'danger' | 'muted';
  size?: 'sm' | 'md' | 'lg';
  align?: 'left' | 'right';
  style?: React.CSSProperties;
}

/** Mono readout of one labelled metric; compose in a row for a HUD strip. */
export function Telemetry(props: TelemetryProps): JSX.Element;
