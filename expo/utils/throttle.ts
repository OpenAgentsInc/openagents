// Simple per-key throttle and debounce helpers for Tinyvex provider usage.
//
// Why these exist
// - The Tinyvex provider reacts to bridge broadcasts (e.g., `tinyvex.update`)
//   which can arrive in bursts while a message is streaming. If we naïvely
//   re-issue a `messages.list` query on every update, we flood the bridge.
// - These helpers let us constrain how often we trigger follow-up queries on
//   a per-thread basis (throttle) and coalesce multiple thread-list updates
//   into a single refresh (debounce).
//
// Design
// - Throttle: for a given key, run at most once per window. If multiple calls
//   happen within the same window, we keep the latest callback and execute it
//   when the window elapses. This favors “latest state” without excess calls.
// - Debounce: for a given key, reset a timer on every call and only run the
//   callback after no new calls arrive for `delayMs`.

type Timer = ReturnType<typeof setTimeout> | null

/**
 * Create a per-key throttle.
 *
 * For each key, executes at most once per `windowMs`.
 * If invoked multiple times during the window, retains the latest callback
 * and executes it when the window elapses.
 */
export function createPerKeyThrottle(windowMs: number) {
  const timers = new Map<string, { timer: Timer; fn?: () => void }>()
  return (key: string, fn: () => void) => {
    const entry = timers.get(key)
    if (entry && entry.timer) {
      // Update latest fn but do not reschedule within the window
      entry.fn = fn
      return
    }
    const obj = { timer: null as Timer, fn }
    const timer = setTimeout(() => {
      try { obj.fn && obj.fn() } finally { timers.delete(key) }
    }, windowMs)
    obj.timer = timer
    timers.set(key, obj)
  }
}

/**
 * Create a per-key debounce.
 *
 * For each key, resets the timer on every call and only runs the callback
 * after `delayMs` have passed without another call for the same key.
 */
export function createPerKeyDebounce(delayMs: number) {
  const timers = new Map<string, Timer>()
  return (key: string, fn: () => void) => {
    const prev = timers.get(key)
    if (prev) clearTimeout(prev)
    const t = setTimeout(() => {
      try { fn() } finally { timers.delete(key) }
    }, delayMs)
    timers.set(key, t)
  }
}
