import { useCallback, useMemo, useRef } from 'react';
import {
  type LayoutConfig,
  LayoutEngine,
  type Point,
  type TreeNode,
} from './layout-engine';
import type { FlowNode } from './types';
import { NODE_SIZES } from './types';
import { TreeConnectionLine } from './TreeConnectionLine';
import { TreeElementNode } from './TreeElementNode';

type TreeLayoutProps = {
  data: FlowNode;
  nodeSpacing?: { x: number; y: number };
  layoutConfig?: Omit<LayoutConfig, 'spacing' | 'direction'> & {
    direction?: 'vertical' | 'horizontal';
  };
  onNodeClick?: (node: FlowNode) => void;
  renderNode: (node: FlowNode, parent?: FlowNode) => React.ReactNode;
  renderConnection?: (
    path: Point[],
    parent: FlowNode,
    child: FlowNode,
  ) => React.ReactNode;
};

export function TreeLayout({
  data,
  nodeSpacing = { x: 50, y: 50 },
  layoutConfig,
  renderNode,
  renderConnection,
  onNodeClick,
}: TreeLayoutProps) {
  const containerRef = useRef<SVGGElement>(null);

  const layoutEngine = useMemo(
    () =>
      new LayoutEngine<FlowNode>({
        spacing: nodeSpacing,
        direction: layoutConfig?.direction ?? 'vertical',
        layout: layoutConfig?.layout,
        connections: layoutConfig?.connections,
      }),
    [nodeSpacing, layoutConfig],
  );

  const parentMap = useMemo(() => {
    const map = new Map<string, FlowNode>();
    const buildMap = (node: TreeNode) => {
      if (node.children) {
        for (const child of node.children) {
          map.set(child.id, node as FlowNode);
          buildMap(child);
        }
      }
    };
    buildMap(data);
    return map;
  }, [data]);

  const allNodes = useMemo(() => layoutEngine.flattenTree(data), [data, layoutEngine]);

  const handleClick = useCallback(
    (e: React.MouseEvent<SVGGElement>) => {
      if (!onNodeClick) return;
      let target = e.target as HTMLElement | SVGElement;
      while (target && target !== e.currentTarget) {
        const nodeId = target.getAttribute('data-node-id');
        if (nodeId) {
          const node = allNodes.find((n) => n.id === nodeId);
          if (node) onNodeClick(node);
          break;
        }
        target = target.parentElement as HTMLElement | SVGElement;
      }
    },
    [onNodeClick, allNodes],
  );

  for (const node of allNodes) {
    const type = node.metadata?.type ?? 'leaf';
    const size = NODE_SIZES[type];
    layoutEngine.setNodeDimension(node.id, size);
  }

  const layout = useMemo(() => layoutEngine.calculate(data), [data, layoutEngine]);

  return (
    <g ref={containerRef} onClick={handleClick}>
      {layout.connections.map((conn) =>
        renderConnection ? (
          <g key={`${conn.parent.id}-${conn.child.id}`}>
            {renderConnection(conn.path, conn.parent, conn.child)}
          </g>
        ) : (
          <TreeConnectionLine
            key={`${conn.parent.id}-${conn.child.id}`}
            path={conn.path}
          />
        ),
      )}
      {layout.nodes.map((positioned) => {
        const parent = parentMap.get(positioned.node.id);
        return (
          <TreeElementNode
            key={positioned.node.id}
            id={positioned.node.id}
            position={positioned.position}
          >
            {renderNode(positioned.node, parent)}
          </TreeElementNode>
        );
      })}
    </g>
  );
}
