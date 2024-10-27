import { clsx } from "clsx"
import { motion } from "framer-motion"
import IconOpenAgents from "../IconOpenAgents"

export function Logo({ className }: { className?: string }) {
  return (
    <motion.div 
      className={clsx(className, 'flex items-center gap-2')}
      whileHover={{ scale: 1.1 }}
      whileTap={{ scale: 0.95 }}
      transition={{ type: "spring", stiffness: 400, damping: 17 }}
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