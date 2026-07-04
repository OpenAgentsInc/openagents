import * as React from 'react'
import { cva, type VariantProps } from 'class-variance-authority'

import { cn } from '@/lib/utils'

const badgeVariants = cva(
  'inline-flex w-fit shrink-0 items-center border px-2 py-0.5 font-mono text-xs font-semibold uppercase leading-none tracking-wide transition-colors',
  {
    variants: {
      variant: {
        default:
          'border-khala-border bg-white/[0.035] text-khala-energy-soft',
        ready:
          'border-khala-success/30 bg-khala-success/10 text-khala-success',
        running:
          'border-khala-energy/30 bg-khala-energy/10 text-khala-energy-soft',
        warning:
          'border-khala-warning/30 bg-khala-warning/10 text-khala-warning',
        danger:
          'border-khala-danger/30 bg-khala-danger/10 text-khala-danger',
        outline: 'border-khala-border bg-transparent text-khala-text-muted',
      },
    },
    defaultVariants: {
      variant: 'default',
    },
  },
)

function Badge({
  className,
  variant,
  ...props
}: React.ComponentProps<'span'> & VariantProps<typeof badgeVariants>) {
  return (
    <span
      className={cn(badgeVariants({ variant }), className)}
      data-slot="badge"
      {...props}
    />
  )
}

export { Badge, badgeVariants }
