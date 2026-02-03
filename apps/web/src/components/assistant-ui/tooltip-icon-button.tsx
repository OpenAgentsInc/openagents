import React, { forwardRef, type ComponentPropsWithRef } from 'react';
import { Slot } from '@radix-ui/react-slot';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

export type TooltipIconButtonProps = ComponentPropsWithRef<typeof Button> & {
  tooltip: string;
  side?: 'top' | 'bottom' | 'left' | 'right';
};

/** Ensures Slot receives exactly one React element (Radix Slot requirement). */
function singleChild(children: React.ReactNode): React.ReactElement {
  const arr = React.Children.toArray(children);
  if (arr.length === 1 && React.isValidElement(arr[0])) return arr[0];
  return <span className="inline-flex size-full items-center justify-center">{children}</span>;
}

export const TooltipIconButton = forwardRef<HTMLButtonElement, TooltipIconButtonProps>(
  ({ children, tooltip, side = 'bottom', className, ...rest }, ref) => {
    return (
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            size="icon"
            {...rest}
            className={cn('aui-button-icon size-6 p-1', className)}
            ref={ref}
          >
            <Slot>{singleChild(children)}</Slot>
            <span className="aui-sr-only sr-only">{tooltip}</span>
          </Button>
        </TooltipTrigger>
        <TooltipContent side={side}>{tooltip}</TooltipContent>
      </Tooltip>
    );
  },
);

TooltipIconButton.displayName = 'TooltipIconButton';
