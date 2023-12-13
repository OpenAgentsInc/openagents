import { usePage } from "@inertiajs/react"
import { StartAuditForm } from "./StartAuditForm"

export const Auditor = () => {
  const props = usePage().props as any

  // if props.flash.message, console.log it
  if (props.flash.message) {
    console.log(props.flash.message)
  }

  return (
    <StartAuditForm />
  )
}
