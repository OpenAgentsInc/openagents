import * as React from 'react'
import { Slot } from '@radix-ui/react-slot'
import { cva, type VariantProps } from 'class-variance-authority'

import { cn } from '@/lib/utils'

const buttonVariants = cva(
  'khala-focus inline-flex min-h-10 shrink-0 items-center justify-center gap-2 whitespace-nowrap border px-4 py-2 font-mono text-sm font-medium transition-colors disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-45',
  {
    variants: {
      variant: {
        default:
          'border-khala-text bg-khala-text text-black hover:bg-white',
        secondary:
          'border-khala-border bg-transparent text-khala-text-muted hover:bg-white/5 hover:text-khala-text',
        ghost:
          'border-transparent bg-transparent text-khala-text-muted hover:bg-white/5 hover:text-khala-text',
        link: 'min-h-0 border-transparent bg-transparent p-0 text-khala-text-faint underline underline-offset-2 hover:text-khala-text',
      },
      size: {
        default: 'min-h-10 px-4 py-2',
        sm: 'min-h-8 px-3 py-1.5 text-xs',
        lg: 'min-h-11 px-5 py-2.5',
      },
    },
    defaultVariants: {
      variant: 'default',
      size: 'default',
    },
  },
)

function Button({
  asChild = false,
  className,
  size,
  variant,
  ...props
}: React.ComponentProps<'button'> &
  VariantProps<typeof buttonVariants> & {
    asChild?: boolean
  }) {
  const Comp = asChild ? Slot : 'button'

  return (
    <Comp
      className={cn(buttonVariants({ variant, size, className }))}
      data-slot="button"
      {...props}
    />
  )
}

export { Button, buttonVariants }
