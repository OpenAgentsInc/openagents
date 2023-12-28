import { FullGestureState, useDrag as useDragHook, UserDragConfig } from '@use-gesture/react'

export function useDrag(handler: (state: FullGestureState<'drag'>) => any, config?: UserDragConfig) {
  return useDragHook((state) => {
    if (state.first) {
      document.body.classList.add('agentgraph__panel__dragged')
    }
    const result = handler(state)
    if (state.last) {
      document.body.classList.remove('agentgraph__panel__dragged')
    }
    return result
  }, config)
}
