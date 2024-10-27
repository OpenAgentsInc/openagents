import { clsx } from "clsx"

export function Gradient({
  className,
  ...props
}: React.ComponentPropsWithoutRef<'div'>) {
  return (
    <div
      {...props}
      className={clsx(
        className,
        'bg-[linear-gradient(115deg,var(--tw-gradient-stops))] from-white/5 from-[28%] via-white/10 via-[70%] to-white/5 sm:bg-[linear-gradient(145deg,var(--tw-gradient-stops))]',
      )}
    />
  )
}

export function GradientBackground() {
  return (
    <div className="relative mx-auto max-w-7xl">
      <div
        className={clsx(
          'absolute -right-60 -top-44 h-60 w-[36rem] transform-gpu md:right-0',
          'bg-[linear-gradient(115deg,var(--tw-gradient-stops))] from-white/5 from-[28%] via-white/10 via-[70%] to-white/5',
          'rotate-[-10deg] rounded-full blur-3xl opacity-50',
        )}
      />
    </div>
  )
}