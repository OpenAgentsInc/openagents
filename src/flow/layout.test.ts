import { describe, it, expect } from 'bun:test'
import { calculateLayout, type LayoutConfig } from './layout.js'
import type { FlowNode } from './model.js'
import { sampleMechaCoderTree, sampleNodeSizes } from './sample-data.js'

describe('calculateLayout', () => {
  const defaultConfig: LayoutConfig = { padding: 8, spacing: 8 }

  it('layouts single node (leaf)', () => {
    const tree: FlowNode = {
      id: 'leaf',
      type: 'task',
      label: 'Leaf'
    }
    const sizes = { leaf: { width: 100, height: 50 } }
    const output = calculateLayout({ root: tree, nodeSizes: sizes, config: defaultConfig })
    expect(output.nodes).toHaveLength(1)
    const node = output.nodes[0]!
    expect(node.x).toBe(0)
    expect(node.y).toBe(0)
    expect(node.size).toEqual({ width: 100, height: 50 })
    expect(output.connections).toHaveLength(0)
  })

  it('layouts vertical parent-child', () => {
    const tree: FlowNode = {
      id: 'parent',
      type: 'container',
      label: 'Vertical Parent',
      direction: 'vertical',
      children: [{
        id: 'child',
        type: 'task',
        label: 'Child'
      }]
    }
    const sizes = {
      parent: { width: 200, height: 120 },
      child: { width: 100, height: 50 }
    }
    const output = calculateLayout({ root: tree, nodeSizes: sizes, config: defaultConfig })
    expect(output.nodes).toHaveLength(2)

    const parent = output.nodes.find(n => n.id === 'parent')!
    expect(parent.x).toBe(0)
    expect(parent.y).toBe(0)

    const child = output.nodes.find(n => n.id === 'child')!
    expect(child.x).toBeCloseTo(8 + (200 - 16 - 100) / 2, 1) // 8 + (184-100)/2 = 8+42=50
    expect(child.y).toBeCloseTo(8, 1)

    expect(output.connections).toHaveLength(1)
    const conn = output.connections[0]!
    expect(conn.parentId).toBe('parent')
    expect(conn.childId).toBe('child')
    expect(conn.waypoints).toHaveLength(2)
  })

  it('layouts horizontal siblings', () => {
    const tree: FlowNode = {
      id: 'parent',
      type: 'container',
      label: 'Horizontal Parent',
      direction: 'horizontal',
      children: [
        { id: 'child1', type: 'task', label: 'Child 1' },
        { id: 'child2', type: 'task', label: 'Child 2' }
      ]
    }
    const sizes = {
      parent: { width: 300, height: 80 },
      child1: { width: 100, height: 50 },
      child2: { width: 100, height: 50 }
    }
    const config: LayoutConfig = { padding: 10, spacing: 10 }
    const output = calculateLayout({ root: tree, nodeSizes: sizes, config })

    const child1 = output.nodes.find(n => n.id === 'child1')!
    const child2 = output.nodes.find(n => n.id === 'child2')!
    // totalRowW = 100+100 +10 =210
    // rowStartX =10 + (280-210)/2 =10+35=45
    expect(child1.x).toBeCloseTo(45, 1)
    expect(child1.y).toBeCloseTo(10 + (60-50)/2, 1) //15
    expect(child2.x).toBeCloseTo(45 +100 +10, 1) //155
    expect(child2.y).toBeCloseTo(child1.y, 1)

    expect(output.connections).toHaveLength(2)
  })

  it('layouts nested tree', () => {
    const tree: FlowNode = {
      id: 'root',
      type: 'root',
      label: 'Root',
      direction: 'vertical',
      children: [{
        id: 'middle',
        type: 'middle',
        label: 'Middle',
        direction: 'horizontal',
        children: [{ id: 'leaf', type: 'leaf', label: 'Leaf' }]
      }]
    }
    const sizes = {
      root: { width: 250, height: 200 },
      middle: { width: 200, height: 100 },
      leaf: { width: 120, height: 60 }
    }
    const output = calculateLayout({ root: tree, nodeSizes: sizes, config: defaultConfig })
    expect(output.nodes).toHaveLength(3)
    expect(output.connections).toHaveLength(2)
  })

  it('layouts sample MechaCoder tree', () => {
    const output = calculateLayout({
      root: sampleMechaCoderTree,
      nodeSizes: sampleNodeSizes,
      config: defaultConfig
    })
    expect(output.nodes.length).toBeGreaterThan(10)
    expect(output.connections.length).toBeGreaterThan(5)
    // root at 0,0
    const rootNode = output.nodes.find(n => n.id === 'root')!
    expect(rootNode.x).toBe(0)
    expect(rootNode.y).toBe(0)
    // no throw
  })

  it('fails on missing node size', () => {
    const tree: FlowNode = { id: 'root', type: 'root', label: 'Root' }
    const sizes: any = {} // missing
    expect(() => calculateLayout({ root: tree, nodeSizes: sizes, config: defaultConfig })).toThrow(/missing size/)
  })

  it('fails on invalid (negative) size', () => {
    const tree: FlowNode = { id: 'root', type: 'root', label: 'Root' }
    const sizes = { root: { width: -1, height: 50 } as any }
    expect(() => calculateLayout({ root: tree, nodeSizes: sizes, config: defaultConfig })).toThrow(/Invalid/)
  })

  it('fails on cycle/duplicate ID', () => {
    const tree: FlowNode = {
      id: 'root',
      type: 'root',
      label: 'Root',
      children: [{ id: 'root', type: 'task', label: 'Duplicate' }] as any
    }
    expect(() => calculateLayout({ root: tree, nodeSizes: { root: {width:100,height:50} }, config: defaultConfig })).toThrow(/Duplicate/)
  })

  it('fails on excessive padding', () => {
    const tree: FlowNode = {
      id: 'parent',
      type: 'container',
      label: 'Parent',
      children: [{ id: 'child', type: 'task', label: 'Child' }]
    }
    const sizes = { parent: { width: 10, height: 10 }, child: { width: 10, height: 10 } }
    const config = { padding: 10, spacing: 0 } // content 10-20=-10
    expect(() => calculateLayout({ root: tree, nodeSizes: sizes, config })).toThrow(/Padding too large/)
  })
})
