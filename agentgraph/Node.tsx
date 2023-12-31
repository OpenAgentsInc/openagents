import React, { useMemo, useState } from 'react'
import { useTransform } from './hooks/useTransform'
import { buildTree } from './tree'
import { Tree, TreeWrapper } from './TreeWrapper'
import { PanelSettingsContext } from './context';
import { globalStyles, styled } from './styles';
import { TitleWithFilter } from './components/Agentgraph/Filter';

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
    const [filter, setFilter] = useState('')
    const [toggled, setToggle] = useState(true)
    const tree = useMemo(() => buildTree(data), [data]) as Tree
    const [rootRef, set, currentPos] = useTransform<HTMLDivElement>()
    const drag = typeof titleBar === 'object' ? titleBar.drag ?? true : true
    const filterEnabled = typeof titleBar === 'object' ? titleBar.filter ?? true : true
    const position = typeof titleBar === 'object' ? titleBar.position || undefined : undefined
    const onDragStart = typeof titleBar === 'object' ? titleBar.onDragStart || undefined : undefined
    const onDragEnd = typeof titleBar === 'object' ? titleBar.onDragEnd || undefined : undefined
    React.useEffect(() => {
      set({ x: position?.x, y: position?.y })
    }, [position, set])

    globalStyles()

    return (
      <PanelSettingsContext.Provider value={{ hideCopyButton }}>
        <PanelMaybe ref={rootRef} className="absolute w-64 rounded-lg shadow-lg">
          {titleBar && (
            <TitleWithFilter
              onDrag={(point) => {
                set(point)
              }}
              drag={drag}
              from={currentPos}
              onDragStart={(point) => onDragStart?.(point)}
              onDragEnd={(point) => onDragEnd?.(point)}
              setFilter={setFilter}
              toggle={(flag?: boolean) => setToggle((t) => flag ?? !t)}
              toggled={toggled}
              title={data.name}
              filterEnabled={false}
            />
          )}
          <div className="p-4">
            <h1 className="text-lg font-semibold">{data.order}. {data.name}</h1>
            <p className="text-sm">{data.description}</p>
            <div className="mt-4">
              <TreeWrapper isRoot tree={tree} toggled={toggled} />
              {/* <pre className="text-xs whitespace-pre-wrap">{JSON.stringify(data, null, 2)}</pre> */}
            </div>
          </div>
        </PanelMaybe>
      </PanelSettingsContext.Provider>

    )
  }
)

const PanelMaybe = styled('div', {
  color: '$highlight2',
  backgroundColor: '$elevation2',
  borderRadius: '4px'
})
