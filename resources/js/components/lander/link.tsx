import { forwardRef } from "react"
import * as Headless from "@headlessui/react"
import { InertiaLinkProps, Link as InertiaLink } from "@inertiajs/react"

export const Link = forwardRef(function Link(
  props: InertiaLinkProps & React.ComponentPropsWithoutRef<'a'>,
  ref: React.ForwardedRef<HTMLAnchorElement>,
) {
  const { target, ...otherProps } = props

  if (target === '_blank') {
    return (
      <Headless.DataInteractive>
        <a 
          ref={ref} 
          target="_blank" 
          rel="noopener noreferrer" 
          {...otherProps} 
        />
      </Headless.DataInteractive>
    )
  }

  return (
    <Headless.DataInteractive>
      <InertiaLink ref={ref} {...props} />
    </Headless.DataInteractive>
  )
})