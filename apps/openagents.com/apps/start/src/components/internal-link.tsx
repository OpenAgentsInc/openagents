import { useRouter } from '@tanstack/react-router'
import { useCallback, useEffect, type ComponentPropsWithoutRef, type MouseEvent } from 'react'

type InternalLinkProps = Omit<ComponentPropsWithoutRef<'a'>, 'href'> & Readonly<{
  href: string
  preload?: 'intent' | 'render' | false
}>

const isPlainPrimaryClick = (event: MouseEvent<HTMLAnchorElement>) =>
  event.button === 0
  && !event.defaultPrevented
  && !event.metaKey
  && !event.ctrlKey
  && !event.shiftKey
  && !event.altKey

export function InternalLink({
  children,
  href,
  onClick,
  onFocus,
  onMouseEnter,
  onTouchStart,
  preload = 'intent',
  target,
  ...props
}: InternalLinkProps) {
  const router = useRouter({ warn: false })

  const preloadRoute = useCallback(() => {
    if (router === undefined || router === null || preload === false) {
      return
    }
    void router.preloadRoute({ to: href as never })
  }, [href, preload, router])

  useEffect(() => {
    if (preload === 'render') {
      preloadRoute()
    }
  }, [preload, preloadRoute])

  return (
    <a
      {...props}
      href={href}
      onClick={event => {
        onClick?.(event)
        if (
          router === undefined
          || router === null
          || target === '_blank'
          || props.download !== undefined
          || !isPlainPrimaryClick(event)
        ) {
          return
        }
        event.preventDefault()
        void router.navigate({ to: href as never })
      }}
      onFocus={event => {
        onFocus?.(event)
        if (!event.defaultPrevented) {
          preloadRoute()
        }
      }}
      onMouseEnter={event => {
        onMouseEnter?.(event)
        if (!event.defaultPrevented) {
          preloadRoute()
        }
      }}
      onTouchStart={event => {
        onTouchStart?.(event)
        if (!event.defaultPrevented) {
          preloadRoute()
        }
      }}
      target={target}
    >
      {children}
    </a>
  )
}
