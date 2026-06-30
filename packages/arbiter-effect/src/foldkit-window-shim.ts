const globalScope = globalThis as unknown as {
  window?: {
    requestAnimationFrame?: (callback: (time: number) => void) => number
    cancelAnimationFrame?: (id: number) => void
  }
}

if (globalScope.window === undefined) {
  let nextFrame = 1
  const timers = new Map<number, ReturnType<typeof setTimeout>>()
  globalScope.window = {
    requestAnimationFrame: callback => {
      const id = nextFrame++
      const timer = setTimeout(() => {
        timers.delete(id)
        callback(performance.now())
      }, 0)
      timers.set(id, timer)
      return id
    },
    cancelAnimationFrame: id => {
      const timer = timers.get(id)
      if (timer !== undefined) {
        clearTimeout(timer)
        timers.delete(id)
      }
    },
  }
}

export {}
