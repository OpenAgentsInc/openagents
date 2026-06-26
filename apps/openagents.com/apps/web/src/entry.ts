import { Runtime } from 'foldkit'

import { Flags, flags, init } from './main'
import { ChangedUrl, ClickedLink, Message } from './message'
import { Model } from './model'
import { installKhalaTokensServedCountUp } from './page/loggedOut/khala-tokens-served-countup-controller'
import { subscriptions } from './subscriptions'
import { update } from './update'
import { view } from './view'

const program = Runtime.makeProgram({
  Model,
  Flags,
  flags,
  init,
  subscriptions,
  update,
  view,
  container: document.getElementById('root'),
  routing: {
    onUrlRequest: request => ClickedLink({ request }),
    onUrlChange: url => ChangedUrl({ url }),
  },
  devTools: {
    Message,
  },
})

Runtime.run(program)

// Smooth, eased, reduced-motion-safe count-up for the public "Khala Tokens
// Served" counter (#6324). Decoupled from the Foldkit loop: a MutationObserver
// eases the displayed digits between the server's ≤3/sec broadcasts.
installKhalaTokensServedCountUp()
