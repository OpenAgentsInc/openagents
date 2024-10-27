import { forwardRef } from "react"
import * as Headless from "@headlessui/react"
import { InertiaLinkProps, Link as InertiaLink } from "@inertiajs/react"

const isExternalUrl = (url: string) => {
  return url.startsWith('http://') || url.startsWith('https://')
}

export const Link = forwardRef(function Link(
  props: InertiaLinkProps & React.ComponentPropsWithoutRef<'a'>,
  ref: React.ForwardedRef<HTMLAnchorElement>,
) {
  const { target, href, ...otherProps } = props

  // Handle external URLs or _blank targets with regular anchor tags
  if (target === '_blank' || (href && isExternalUrl(href))) {
    return (
      <Headless.DataInteractive>
        <a 
          ref={ref} 
          href={href}
          target={target || '_blank'} 
          rel="noopener noreferrer" 
          {...otherProps} 
        />
      </Headless.DataInteractive>
    )
  }

  return (
    <Headless.DataInteractive>
      <InertiaLink ref={ref} href={href} {...otherProps} />
    </Headless.DataInteractive>
  )
})