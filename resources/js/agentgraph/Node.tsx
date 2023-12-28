import React from 'react'

interface NodeProps {
  data: any
  position?: [number, number]
}

export const Node = React.memo(
  ({
    data,
    position: undefined
  }: NodeProps) => {
    return (
      <div className="w-64 h-64 bg-gray-200 rounded-lg shadow-lg">
        <div className="p-4">
          <h1 className="text-lg font-semibold">{data.name}</h1>
          <p className="text-sm">{data.description}</p>
          {/* Then just dump the rest as a JSON stringified object */}
          <div className="mt-4">
            <pre className="text-xs whitespace-pre-wrap">{JSON.stringify(data, null, 2)}</pre>
          </div>
        </div>
      </div>
    )
  }
)
