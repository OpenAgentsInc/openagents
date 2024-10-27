import { clsx } from "clsx"
import { motion } from "framer-motion"
import { Subheading } from "./text"

export function BentoCard({
  dark = false,
  className = '',
  eyebrow,
  title,
  description,
  graphic,
  fade = [],
}: {
  dark?: boolean
  className?: string
  eyebrow: React.ReactNode
  title: React.ReactNode
  description: React.ReactNode
  graphic: React.ReactNode
  fade?: ('top' | 'bottom')[]
}) {
  return (
    <motion.div
      initial="idle"
      whileHover="active"
      variants={{ idle: {}, active: {} }}
      data-dark={dark ? 'true' : undefined}
      className={clsx(
        className,
        'group relative flex flex-col overflow-hidden rounded-lg',
        'bg-card shadow-sm ring-1 ring-border',
        'data-[dark]:bg-background data-[dark]:ring-border',
      )}
    >
      <div className="relative h-80 shrink-0">
        {graphic}
        {fade.includes('top') && (
          <div className="absolute inset-0 bg-gradient-to-b from-card to-50% group-data-[dark]:from-background group-data-[dark]:from-[-25%]" />
        )}
        {fade.includes('bottom') && (
          <div className="absolute inset-0 bg-gradient-to-t from-card to-50% group-data-[dark]:from-background group-data-[dark]:from-[-25%]" />
        )}
      </div>
      <div className="relative p-10">
        <Subheading as="h3" dark={dark}>
          {eyebrow}
        </Subheading>
        <p className="mt-1 text-2xl/8 font-medium tracking-tight text-foreground">
          {title}
        </p>
        <p className="mt-2 max-w-[600px] text-sm/6 text-muted-foreground">
          {description}
        </p>
      </div>
    </motion.div>
  )
}