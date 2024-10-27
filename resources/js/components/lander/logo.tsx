import { clsx } from "clsx"
import { motion } from "framer-motion"
import IconOpenAgents from "../IconOpenAgents"

export function Logo({ className }: { className?: string }) {
  return (
    <div className={clsx(className, 'flex items-center gap-2')}>
      <motion.div
        whileHover={{ scale: 1.1 }}
        whileTap={{ scale: 0.95 }}
        transition={{ type: "spring", stiffness: 400, damping: 17 }}
      >
        <IconOpenAgents className="w-6 h-6" />
      </motion.div>
      <motion.span
        className="text-2xl font-bold"
        initial={{ opacity: 0, x: -20 }}
        animate={{ opacity: 1, x: 0 }}
        transition={{ duration: 0.5 }}
      >
        OpenAgents
      </motion.span>
    </div>
  )
}

export function Mark({ className }: { className?: string }) {
  return <IconOpenAgents className={className} />
}
