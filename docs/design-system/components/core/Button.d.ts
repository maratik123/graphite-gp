import React from 'react';

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';
export type ButtonSize = 'sm' | 'md' | 'lg';

/**
 * Button props.
 * @startingPoint section="Core" subtitle="Button — primary / secondary / ghost / danger" viewport="700x150"
 */
export interface ButtonProps {
  children?: React.ReactNode;
  /** primary = GP vermilion; secondary = hairline paper; ghost = chromeless; danger = destructive/crash. */
  variant?: ButtonVariant;
  size?: ButtonSize;
  disabled?: boolean;
  iconLeft?: React.ReactNode;
  iconRight?: React.ReactNode;
  fullWidth?: boolean;
  type?: 'button' | 'submit' | 'reset';
  onClick?: (e: React.MouseEvent<HTMLButtonElement>) => void;
  style?: React.CSSProperties;
}

/** Primary interactive control for Graphite GP. */
export function Button(props: ButtonProps): JSX.Element;
