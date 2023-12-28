import { useRef, useCallback } from 'react'

export function useTransform<T extends HTMLElement>(): [
  React.RefObject<T>,
  (point: { x?: number; y?: number }) => void,
  { x: number; y: number }
] {
  const ref = useRef<T>(null)
  const local = useRef({ x: 0, y: 0 })

  const set = useCallback((point: { x?: number; y?: number }) => {
    Object.assign(local.current, point)
    if (ref.current) ref.current.style.transform = `translate3d(${local.current.x}px, ${local.current.y}px, 0)`
  }, [])

  return [ref, set, { get x() { return local.current.x }, get y() { return local.current.y }}]
}
