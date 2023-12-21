// import { DataInteractive as HeadlessDataInteractive } from '@headlessui/react'
import { Link as InertiaLink, type InertiaLinkProps } from '@inertiajs/react'
import React from 'react'

export const Link = React.forwardRef(function Link(
  props: InertiaLinkProps,
  ref: React.ForwardedRef<HTMLAnchorElement>
) {
  return (
    // <HeadlessDataInteractive>
    <InertiaLink {...props} ref={ref} />
    // </HeadlessDataInteractive>
  )
})
