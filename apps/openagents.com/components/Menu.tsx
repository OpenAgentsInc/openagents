"use client";

import React, { type ReactNode } from 'react';
import { cx } from '@arwes/react';

interface MenuProps {
  className?: string;
  children: ReactNode;
}

export const Menu = (props: MenuProps): React.ReactElement => {
  const { className, children } = props;

  return (
    <ul 
      className={cx(
        'flex flex-row gap-2 list-none m-0 p-0',
        className
      )}
    >
      {children}
    </ul>
  );
};