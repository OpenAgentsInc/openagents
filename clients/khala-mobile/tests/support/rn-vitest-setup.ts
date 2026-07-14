;(globalThis as Record<string, unknown>).__DEV__ ??= false
;(globalThis as Record<string, unknown>).IS_REACT_ACT_ENVIRONMENT ??= true
;(globalThis as Record<string, unknown>).expo ??= {
  EventEmitter: class EventEmitter {
    addListener() { return { remove() {} } }
    emit() {}
    removeAllListeners() {}
    removeSubscription() {}
  },
}
