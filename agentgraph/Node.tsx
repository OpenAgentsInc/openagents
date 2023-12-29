import React, { useMemo } from 'react'
import { useTransform } from './hooks/useTransform'
import { TitleBar } from './TitleBar';
import { buildTree } from './tree'
import { Tree, TreeWrapper } from './TreeWrapper'
import { PanelSettingsContext } from './context';
import { globalStyles } from './styles';

interface NodeProps {
  data: any // All data passed to the node will be rendered
  position?: { x: number; y: number }
  titleBar: any
  hideCopyButton?: boolean
}

export const Node = React.memo(
  ({
    data,
    titleBar = {
      drag: true,
      position: undefined,
      onDrag: undefined,
      onDragStart: undefined,
      onDragEnd: undefined,
    },
    hideCopyButton = false,
  }: NodeProps) => {
    const tree = useMemo(() => buildTree(data), [data]) as Tree
    const [rootRef, set, currentPos] = useTransform<HTMLDivElement>()
    const drag = typeof titleBar === 'object' ? titleBar.drag ?? true : true
    const position = typeof titleBar === 'object' ? titleBar.position || undefined : undefined
    const onDragStart = typeof titleBar === 'object' ? titleBar.onDragStart || undefined : undefined
    const onDragEnd = typeof titleBar === 'object' ? titleBar.onDragEnd || undefined : undefined
    React.useEffect(() => {
      set({ x: position?.x, y: position?.y })
    }, [position, set])

    globalStyles()

    return (
      <PanelSettingsContext.Provider value={{ hideCopyButton }}>
        <div ref={rootRef} className="absolute w-64 bg-gray-200 rounded-lg shadow-lg">
          {titleBar && (
            <TitleBar
              onDrag={(point) => {
                set(point)
              }}
              drag={drag}
              from={currentPos}
              onDragStart={(point) => onDragStart?.(point)}
              onDragEnd={(point) => onDragEnd?.(point)}
            />
          )}
          <div className="p-4">
            <h1 className="text-lg font-semibold">{data.order}. {data.name}</h1>
            <p className="text-sm">{data.description}</p>
            <div className="mt-4">
              <TreeWrapper tree={tree} />
              {/* <pre className="text-xs whitespace-pre-wrap">{JSON.stringify(data, null, 2)}</pre> */}
            </div>
          </div>
        </div>
      </PanelSettingsContext.Provider>
    )
  }
)
