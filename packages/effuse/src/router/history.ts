/**
 * Minimal history abstraction.
 *
 * We target browser history first, but keep this interface tiny so a Worker /
 * server host can adapt it later.
 */

export interface History {
  readonly current: () => URL
  readonly push: (url: URL) => void
  readonly replace: (url: URL) => void
  readonly listen: (listener: (url: URL) => void) => () => void
}

const hrefFromUrl = (url: URL): string => `${url.pathname}${url.search}${url.hash}`

export const BrowserHistory: History = {
  current: () => new URL(window.location.href),
  push: (url) => {
    window.history.pushState({}, "", hrefFromUrl(url))
  },
  replace: (url) => {
    window.history.replaceState({}, "", hrefFromUrl(url))
  },
  listen: (listener) => {
    const onPopState = () => {
      listener(new URL(window.location.href))
    }
    window.addEventListener("popstate", onPopState)
    return () => window.removeEventListener("popstate", onPopState)
  },
}

