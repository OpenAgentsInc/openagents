import type { Html } from 'foldkit/html'

import * as Ui from './ui'

export const notFoundView = (
  path: string,
  backLinkHref: string,
  backLinkText: string,
): Html => {
  return Ui.container([
    Ui.emptyState({
      title: 'Page not found',
      body: `The path "${path}" was not found.`,
      action: Ui.linkButton({ href: backLinkHref, label: backLinkText }),
    }),
  ])
}
