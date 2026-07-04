import * as React from 'react'

import { cn } from './classes'

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger'
export type ButtonSize = 'sm' | 'md' | 'icon'

export type ButtonProps = Omit<
  React.ButtonHTMLAttributes<HTMLButtonElement>,
  'className'
> & Readonly<{
  className?: string
  size?: ButtonSize
  variant?: ButtonVariant
}>

const buttonBase =
  'relative inline-flex shrink-0 items-center justify-center gap-2 rounded-oa-lg border font-medium tracking-normal transition-colors duration-150 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-oa-accent disabled:cursor-not-allowed disabled:opacity-45'

const buttonVariants: Record<ButtonVariant, string> = {
  primary:
    'border-oa-accent bg-oa-accent text-black hover:border-oa-accent-soft hover:bg-oa-accent-soft',
  secondary:
    'border-oa-border bg-oa-surface text-oa-text hover:border-oa-accent hover:bg-oa-surface-active',
  ghost:
    'border-transparent bg-transparent text-oa-text-muted hover:border-oa-border-strong hover:bg-oa-surface',
  danger:
    'border-oa-danger bg-oa-danger text-white hover:border-oa-danger-hover hover:bg-oa-danger-hover',
}

const buttonSizes: Record<ButtonSize, string> = {
  sm: 'h-8 px-2.5 text-sm',
  md: 'h-9 px-3 text-sm',
  icon: 'size-9 p-0 text-sm',
}

export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  function Button(
    {
      children,
      className,
      size = 'md',
      type = 'button',
      variant = 'primary',
      ...props
    },
    ref,
  ) {
    return (
      <button
        ref={ref}
        className={cn(
          buttonBase,
          buttonVariants[variant],
          buttonSizes[size],
          className,
        )}
        type={type}
        {...props}
      >
        {size === 'icon' ? (
          <span
            aria-hidden="true"
            className="pointer-fine:hidden pointer-events-none absolute left-1/2 top-1/2 size-[max(100%,3rem)] -translate-x-1/2 -translate-y-1/2"
          />
        ) : null}
        {children}
      </button>
    )
  },
)

export type AnchorButtonProps = Omit<
  React.AnchorHTMLAttributes<HTMLAnchorElement>,
  'className'
> & Readonly<{
  className?: string
  size?: Exclude<ButtonSize, 'icon'>
  variant?: ButtonVariant
}>

export const AnchorButton = React.forwardRef<
  HTMLAnchorElement,
  AnchorButtonProps
>(function AnchorButton(
  {
    children,
    className,
    size = 'md',
    variant = 'secondary',
    ...props
  },
  ref,
) {
  return (
    <a
      ref={ref}
      className={cn(
        buttonBase,
        buttonVariants[variant],
        buttonSizes[size],
        className,
      )}
      {...props}
    >
      {children}
    </a>
  )
})

export type PanelProps = React.HTMLAttributes<HTMLElement> & Readonly<{
  bleed?: boolean
}>

export const Panel = React.forwardRef<HTMLElement, PanelProps>(
  function Panel({ bleed = false, children, className, ...props }, ref) {
    return (
      <section
        ref={ref}
        className={cn(
          'rounded-oa-xl border border-oa-border bg-oa-surface text-oa-text-body',
          bleed ? 'p-0' : 'p-4 sm:p-5',
          className,
        )}
        {...props}
      >
        {children}
      </section>
    )
  },
)

export const PanelHeader = ({
  children,
  className,
  ...props
}: React.HTMLAttributes<HTMLDivElement>): React.JSX.Element => (
  <div
    className={cn(
      'mb-4 flex flex-col gap-1 border-b border-oa-border-muted pb-3 sm:flex-row sm:items-end sm:justify-between',
      className,
    )}
    {...props}
  >
    {children}
  </div>
)

export type CardProps = React.HTMLAttributes<HTMLElement>

export const Card = React.forwardRef<HTMLElement, CardProps>(
  function Card({ children, className, ...props }, ref) {
    return (
      <article
        ref={ref}
        className={cn(
          'rounded-oa-lg border border-oa-border bg-oa-surface-raised p-4 text-oa-text-body',
          className,
        )}
        {...props}
      >
        {children}
      </article>
    )
  },
)

export const CardHeader = ({
  children,
  className,
  ...props
}: React.HTMLAttributes<HTMLDivElement>): React.JSX.Element => (
  <div className={cn('mb-3 flex flex-col gap-1', className)} {...props}>
    {children}
  </div>
)

export const CardTitle = ({
  children,
  className,
  ...props
}: React.HTMLAttributes<HTMLHeadingElement>): React.JSX.Element => (
  <h3
    className={cn('text-base font-medium tracking-normal text-oa-text', className)}
    {...props}
  >
    {children}
  </h3>
)

export const CardDescription = ({
  children,
  className,
  ...props
}: React.HTMLAttributes<HTMLParagraphElement>): React.JSX.Element => (
  <p className={cn('text-base/7 text-oa-text-muted sm:text-sm/6', className)} {...props}>
    {children}
  </p>
)

export type FieldProps = Omit<
  React.InputHTMLAttributes<HTMLInputElement>,
  'className' | 'name'
> & Readonly<{
  className?: string
  description?: string
  error?: string
  label: React.ReactNode
  name: string
}>

export const TextField = React.forwardRef<HTMLInputElement, FieldProps>(
  function TextField(
    { className, description, error, id, label, name, type = 'text', ...props },
    ref,
  ) {
    const generatedId = React.useId()
    const inputId = id ?? generatedId
    const descriptionId =
      description === undefined ? undefined : `${inputId}-description`
    const errorId = error === undefined ? undefined : `${inputId}-error`
    const describedBy = [descriptionId, errorId]
      .filter((value): value is string => value !== undefined)
      .join(' ')

    return (
      <div className={cn('grid gap-2', className)}>
        <label className="text-base/7 font-medium text-oa-text sm:text-sm/6" htmlFor={inputId}>
          {label}
        </label>
        {description === undefined ? null : (
          <p className="text-base/7 text-oa-text-muted sm:text-sm/6" id={descriptionId}>
            {description}
          </p>
        )}
        <input
          ref={ref}
          aria-describedby={describedBy === '' ? undefined : describedBy}
          aria-invalid={error === undefined ? undefined : true}
          className="h-11 w-full min-w-0 rounded-oa-md border border-oa-border bg-oa-surface px-3 text-base text-oa-text outline-none transition-colors placeholder:text-oa-text-faint focus:border-oa-accent focus:outline-2 focus:-outline-offset-1 focus:outline-oa-accent sm:h-9 sm:text-sm"
          id={inputId}
          name={name}
          type={type}
          {...props}
        />
        {error === undefined ? null : (
          <p className="text-base/7 text-oa-danger sm:text-sm/6" id={errorId}>
            {error}
          </p>
        )}
      </div>
    )
  },
)

export type TextareaFieldProps = Omit<
  React.TextareaHTMLAttributes<HTMLTextAreaElement>,
  'className' | 'name'
> & Readonly<{
  className?: string
  description?: string
  error?: string
  label: React.ReactNode
  name: string
}>

export const TextareaField = React.forwardRef<
  HTMLTextAreaElement,
  TextareaFieldProps
>(function TextareaField(
  { className, description, error, id, label, name, rows = 4, ...props },
  ref,
) {
  const generatedId = React.useId()
  const inputId = id ?? generatedId
  const descriptionId =
    description === undefined ? undefined : `${inputId}-description`
  const errorId = error === undefined ? undefined : `${inputId}-error`
  const describedBy = [descriptionId, errorId]
    .filter((value): value is string => value !== undefined)
    .join(' ')

  return (
    <div className={cn('grid gap-2', className)}>
      <label className="text-base/7 font-medium text-oa-text sm:text-sm/6" htmlFor={inputId}>
        {label}
      </label>
      {description === undefined ? null : (
        <p className="text-base/7 text-oa-text-muted sm:text-sm/6" id={descriptionId}>
          {description}
        </p>
      )}
      <textarea
        ref={ref}
        aria-describedby={describedBy === '' ? undefined : describedBy}
        aria-invalid={error === undefined ? undefined : true}
        className="min-h-32 w-full min-w-0 rounded-oa-md border border-oa-border bg-oa-surface px-3 py-2 text-base text-oa-text outline-none transition-colors placeholder:text-oa-text-faint focus:border-oa-accent focus:outline-2 focus:-outline-offset-1 focus:outline-oa-accent sm:text-sm"
        id={inputId}
        name={name}
        rows={rows}
        {...props}
      />
      {error === undefined ? null : (
        <p className="text-base/7 text-oa-danger sm:text-sm/6" id={errorId}>
          {error}
        </p>
      )}
    </div>
  )
})

export type NavItem = Readonly<{
  current?: boolean
  href: string
  label: string
}>

export type TopNavProps = React.HTMLAttributes<HTMLElement> & Readonly<{
  actions?: React.ReactNode
  brand: React.ReactNode
  items: ReadonlyArray<NavItem>
}>

const navLinkClass = (current: boolean | undefined): string =>
  cn(
    'rounded-oa-md px-3 py-2 text-sm transition-colors',
    current === true
      ? 'bg-oa-surface-active text-oa-text'
      : 'text-oa-text-muted hover:bg-oa-surface hover:text-oa-text',
  )

export const TopNav = React.forwardRef<HTMLElement, TopNavProps>(
  function TopNav({ actions, brand, className, items, ...props }, ref) {
    return (
      <header
        ref={ref}
        className={cn(
          'border-b border-oa-border bg-oa-bg text-oa-text',
          className,
        )}
        {...props}
      >
        <div className="mx-auto flex min-h-14 w-full max-w-6xl items-center justify-between gap-3 px-4">
          <div className="font-oa-mono text-sm text-oa-accent">{brand}</div>
          <nav aria-label="Primary" className="hidden items-center gap-1 lg:flex">
            {items.map(item => (
              <a
                aria-current={item.current === true ? 'page' : undefined}
                className={navLinkClass(item.current)}
                href={item.href}
                key={item.href}
              >
                {item.label}
              </a>
            ))}
          </nav>
          <div className="hidden items-center gap-2 lg:flex">{actions}</div>
          <details className="group relative lg:hidden">
            <summary className="list-none rounded-oa-md border border-oa-border bg-oa-surface px-3 py-2 text-sm text-oa-text [&::-webkit-details-marker]:hidden">
              Menu
            </summary>
            <div className="absolute right-0 z-20 mt-2 grid min-w-48 gap-1 rounded-oa-lg border border-oa-border bg-oa-bg p-2">
              {items.map(item => (
                <a
                  aria-current={item.current === true ? 'page' : undefined}
                  className={navLinkClass(item.current)}
                  href={item.href}
                  key={item.href}
                >
                  {item.label}
                </a>
              ))}
              {actions === undefined ? null : (
                <div className="mt-2 border-t border-oa-border-muted pt-2">{actions}</div>
              )}
            </div>
          </details>
        </div>
      </header>
    )
  },
)

export type CodeBlockProps = React.HTMLAttributes<HTMLDivElement> & Readonly<{
  code: string
  filename?: string
  language?: string
}>

export const CodeBlock = React.forwardRef<HTMLDivElement, CodeBlockProps>(
  function CodeBlock({ className, code, filename, language, ...props }, ref) {
    return (
      <div
        ref={ref}
        className={cn(
          'overflow-hidden rounded-oa-lg border border-oa-border bg-oa-bg font-oa-mono text-sm text-oa-code',
          className,
        )}
        {...props}
      >
        {filename === undefined && language === undefined ? null : (
          <div className="flex items-center justify-between border-b border-oa-border-muted px-3 py-2 text-xs text-oa-code-muted">
            <span>{filename}</span>
            <span>{language}</span>
          </div>
        )}
        <pre className="overflow-x-auto p-3">
          <code>{code}</code>
        </pre>
      </div>
    )
  },
)
