// Simple per-key throttle and debounce helpers for Tinyvex provider usage.

type Timer = ReturnType<typeof setTimeout> | null

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

