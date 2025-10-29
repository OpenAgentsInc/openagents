import { create } from 'zustand'

export type ToastType = 'info' | 'success' | 'error'
export type Toast = { id: string; text: string; type: ToastType; duration: number }

type ToastState = {
  toasts: Toast[]
  enqueue: (t: Omit<Toast, 'id'> & { id?: string }) => string
  remove: (id: string) => void
  clear: () => void
}

export const useToastStore = create<ToastState>((set, get) => ({
  toasts: [],
  enqueue: (t) => {
    const id = t.id || `${Date.now()}-${Math.random().toString(36).slice(2)}`
    const next: Toast = { id, text: t.text, type: t.type || 'info', duration: Math.max(800, t.duration || 1800) }
    set({ toasts: [...get().toasts, next].slice(-4) }) // keep last 4
    return id
  },
  remove: (id) => set({ toasts: get().toasts.filter((x) => x.id !== id) }),
  clear: () => set({ toasts: [] }),
}))

export function toast(text: string, opts?: { type?: ToastType; duration?: number }) {
  const id = useToastStore.getState().enqueue({ text, type: opts?.type || 'info', duration: opts?.duration || 1800 })
  const ms = Math.max(800, opts?.duration || 1800)
  try { setTimeout(() => useToastStore.getState().remove(id), ms) } catch {}
}

