import React from 'react'
import { useTransform } from './hooks/useTransform'
import { TitleBar } from './TitleBar';

interface NodeProps {
  data: any
  position?: { x: number; y: number }
}

export const Node = React.memo(
  ({
    data,
    position = undefined
  }: NodeProps) => {
    const [rootRef, set] = useTransform<HTMLDivElement>()
    React.useEffect(() => {
      set({ x: position?.x, y: position?.y })
    }, [position, set])
    return (
      <div ref={rootRef} className="absolute w-64 bg-gray-200 rounded-lg shadow-lg">
        <TitleBar
          onDrag={(point) => {
            // console.log(point)
            set(point)
          }}
        />
        <div className="p-4">
          <h1 className="text-lg font-semibold">{data.order}. {data.name}</h1>
          <p className="text-sm">{data.description}</p>
          <div className="mt-4">
            <pre className="text-xs whitespace-pre-wrap">{JSON.stringify(data, null, 2)}</pre>
          </div>
        </div>
      </div>
    )
  }
)
