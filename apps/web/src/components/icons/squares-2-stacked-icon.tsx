import clsx from 'clsx';
import type { ComponentProps } from 'react';

export function Squares2StackedIcon({ className, ...props }: ComponentProps<'svg'>) {
  return (
    <svg
      width={13}
      height={13}
      viewBox="0 0 13 13"
      fill="none"
      strokeWidth={1}
      role="image"
      className={clsx('inline-block', className)}
      {...props}
    >
      <path
        d="M12.5 11.5V5.5C12.5 4.94772 12.0523 4.5 11.5 4.5H8.5V7.5C8.5 8.05228 8.05228 8.5 7.5 8.5H4.5V11.5C4.5 12.0523 4.94772 12.5 5.5 12.5H11.5C12.0523 12.5 12.5 12.0523 12.5 11.5Z"
        fill="currentColor"
        fillOpacity="0.2"
      />
      <path
        d="M0.5 1.5C0.5 0.947715 0.947715 0.5 1.5 0.5H7.5C8.05228 0.5 8.5 0.947715 8.5 1.5V7.5C8.5 8.05228 8.05228 8.5 7.5 8.5H1.5C0.947715 8.5 0.5 8.05228 0.5 7.5V1.5Z"
        stroke="currentColor"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path
        d="M8.5 4.5H11.5C12.0523 4.5 12.5 4.94772 12.5 5.5V11.5C12.5 12.0523 12.0523 12.5 11.5 12.5H5.5C4.94772 12.5 4.5 12.0523 4.5 11.5V8.5"
        stroke="currentColor"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
