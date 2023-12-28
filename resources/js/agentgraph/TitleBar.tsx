import React, { useRef } from 'react'

interface TitleBarProps {
  onDrag: (point: { x?: number; y?: number }) => void
  onDragStart: (point: { x?: number; y?: number }) => void
  onDragEnd: (point: { x?: number; y?: number }) => void
  title: React.ReactNode
  drag: boolean
  filterEnabled: boolean
  from?: { x?: number; y?: number }
}

export function TitleBar({
  onDrag,
  onDragStart,
  onDragEnd,
  title,
  drag,
  filterEnabled,
  from,
}: TitleBarProps) {
  const inputRef = useRef<HTMLInputElement>(null)

  const bind = useDrag(
    ({ offset: [x, y], first, last }) => {
      onDrag({ x, y })

      if (first) {
        onDragStart({ x, y })
      }

      if (last) {
        onDragEnd({ x, y })
      }
    },
    {
      filterTaps: true,
      from: ({ offset: [x, y] }) => [from?.x || x, from?.y || y],
    }
  )

  return (
    <div  {...(drag ? bind() : {})} drag={drag} className="flex items-center justify-between px-4 py-2 bg-gray-300 rounded-t-lg">
      <div className="flex items-center">
        <div className="w-3 h-3 mr-2 bg-red-500 rounded-full"></div>
        <div className="w-3 h-3 mr-2 bg-yellow-500 rounded-full"></div>
        <div className="w-3 h-3 mr-2 bg-green-500 rounded-full"></div>
      </div>
      <span>{title}</span>
      <div className="flex items-center">
        <div className="mr-2">
          <input
            ref={inputRef}
            type="checkbox"
            checked={filterEnabled}
            onChange={() => { }}
            className="w-4 h-4"
          />
        </div>
        <div className="mr-2">
          <input
            type="checkbox"
            checked={drag}
            onChange={() => { }}
            className="w-4 h-4"
          />
        </div>
        <div className="mr-2">
          <button className="w-4 h-4">
            <svg
              className="w-4 h-4"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            >
              <line x1="12" y1="5" x2="12" y2="19"></line>
              <line x1="5" y1="12" x2="19" y2="12"></line>
            </svg>
          </button>
        </div>
        <div className="mr-2">
          <button className="w-4 h-4">
            <svg

              className="w-4 h-4"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            >
              <polygon points="5 3 19 12 5 21 5 3"></polygon>
            </svg>
          </button>
        </div>
      </div>
    </div>
  )
}
