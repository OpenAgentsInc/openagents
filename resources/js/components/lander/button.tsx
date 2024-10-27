import * as Headless from '@headlessui/react'
import { clsx } from 'clsx'
import { Link } from './link'

const variants = {
  primary: clsx(
    'inline-flex items-center justify-center px-4 py-[calc(theme(spacing.2)-1px)]',
    'rounded-full border border-transparent bg-primary shadow-md',
    'whitespace-nowrap text-base font-medium text-primary-foreground',
    'data-[disabled]:bg-primary data-[hover]:bg-primary/90 data-[disabled]:opacity-40',
  ),
  secondary: clsx(
    'relative inline-flex items-center justify-center px-4 py-[calc(theme(spacing.2)-1px)]',
    'rounded-full border border-transparent bg-secondary shadow-md ring-1 ring-border',
    'after:absolute after:inset-0 after:rounded-full after:shadow-[inset_0_0_2px_1px_rgba(255,255,255,0.3)]',
    'whitespace-nowrap text-base font-medium text-secondary-foreground',
    'data-[disabled]:bg-secondary data-[hover]:bg-secondary/80 data-[disabled]:opacity-40',
  ),
  outline: clsx(
    'inline-flex items-center justify-center px-2 py-[calc(theme(spacing.[1.5])-1px)]',
    'rounded-lg border border-transparent shadow ring-1 ring-border',
    'whitespace-nowrap text-sm font-medium text-foreground',
    'data-[disabled]:bg-transparent data-[hover]:bg-accent data-[disabled]:opacity-40',
  ),
}

type ButtonProps = {
  variant?: keyof typeof variants
} & (
  | React.ComponentPropsWithoutRef<typeof Link>
  | (Headless.ButtonProps & { href?: undefined })
)

export function Button({
  variant = 'primary',
  className,
  ...props
}: ButtonProps) {
  className = clsx(className, variants[variant])

  if (typeof props.href === 'undefined') {
    return <Headless.Button {...props} className={className} />
  }

  return <Link {...props} className={className} />
}