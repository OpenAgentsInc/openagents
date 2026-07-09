export type AvatarVideoElementLatch = Readonly<{
  acquire: () => Promise<HTMLVideoElement>
  supply: (element: HTMLVideoElement) => void
  clear: (element: HTMLVideoElement) => void
  dispose: () => void
}>

/**
 * Cancellable bridge from the Effect Native host driver to the imperative
 * avatar session. Disposal rejects every pending/future acquire with fixed
 * copy so an unmounted surface cannot leave a start promise parked forever.
 */
export function makeAvatarVideoElementLatch(): AvatarVideoElementLatch {
  let element: HTMLVideoElement | null = null
  let disposed = false
  let waiters: Array<Readonly<{
    resolve: (element: HTMLVideoElement) => void
    reject: (error: Error) => void
  }>> = []

  return {
    acquire: () => {
      if (disposed) {
        return Promise.reject(new Error("sarah_avatar_video_latch_disposed"))
      }
      if (element !== null) return Promise.resolve(element)
      return new Promise((resolve, reject) => {
        waiters.push({ resolve, reject })
      })
    },
    supply: (nextElement) => {
      if (disposed) return
      element = nextElement
      for (const waiter of waiters.splice(0)) waiter.resolve(nextElement)
    },
    clear: (removedElement) => {
      if (element === removedElement) element = null
    },
    dispose: () => {
      if (disposed) return
      disposed = true
      element = null
      const error = new Error("sarah_avatar_video_latch_disposed")
      for (const waiter of waiters.splice(0)) waiter.reject(error)
    },
  }
}
