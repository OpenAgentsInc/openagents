import { clsx } from "clsx"
import { motion } from "framer-motion"
import { IconOpenAgents } from "@/components/ui/icons"

export function Logo({ className }: { className?: string }) {
  return (
    <motion.div
      className={clsx(className, 'flex items-center gap-2')}
      whileHover={{ scale: 1.03 }}
      whileTap={{ scale: 0.98 }}
      transition={{ type: "spring", stiffness: 300, damping: 25 }}
    >
      <IconOpenAgents className="w-6 h-6" />
      <span className="text-2xl font-bold">
        OpenAgents
      </span>
    </motion.div>
  )
}

export function Mark({ className }: { className?: string }) {
  return <IconOpenAgents className={className} />
}
