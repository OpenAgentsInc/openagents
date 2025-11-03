import { create } from 'zustand'

type Toast = { id: string; text: string; type: 'info'|'success'|'error'; expiresAt: number }
type ToastState = {
  toasts: Toast[]
  enqueue: (t: Omit<Toast,'id'|'expiresAt'> & { id?: string; duration?: number }) => string
  remove: (id: string) => void
}

export const useToastStore = create<ToastState>((set, get) => ({
  toasts: [],
  enqueue: ({ id, text, type, duration }) => {
    const toastId = id || `${Date.now()}-${Math.random().toString(36).slice(2)}`
    const ttl = typeof duration === 'number' ? duration : 1800
    set((s) => ({ toasts: [...s.toasts, { id: toastId, text, type, expiresAt: Date.now() + ttl }] }))
    try { setTimeout(() => get().remove(toastId), ttl) } catch {}
    return toastId
  },
  remove: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}))

