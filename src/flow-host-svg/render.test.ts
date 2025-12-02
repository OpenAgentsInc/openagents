import { describe, it, expect } from "bun:test"
import {
  renderLayout,
  renderFlowSVG,
  renderToSVGString,
  getCanvasTransform,
  svgElementToString,
  DEFAULT_RENDER_CONFIG,
  type SVGGroup,
  type SVGRect,
  type SVGText,
  type SVGPath,
} from "./render.js"
import { calculateLayout, type LayoutOutput } from "../flow/layout.js"
import { initialCanvasState, type CanvasState } from "../flow/canvas.js"
import { sampleMechaCoderTree, sampleNodeSizes } from "../flow/sample-data.js"
import type { PositionedNode } from "../flow/model.js"

describe("renderLayout", () => {
  const simpleLayout: LayoutOutput = {
    nodes: [
      {
        id: "root",
        type: "root",
        label: "Root Node",
        x: 0,
        y: 0,
        size: { width: 100, height: 50 },
      } as PositionedNode,
      {
        id: "child",
        type: "task",
        label: "Child Node",
        x: 0,
        y: 70,
        size: { width: 100, height: 50 },
      } as PositionedNode,
    ],
    connections: [
      {
        parentId: "root",
        childId: "child",
        waypoints: [
          { x: 50, y: 50 },
          { x: 50, y: 70 },
        ],
      },
    ],
  }

  it("renders nodes as rect + text groups", () => {
    const result = renderLayout(simpleLayout)
    
    expect(result.type).toBe("g")
    expect(result.className).toBe("flow-content")
    
    // 1 connection + 2 node groups
    expect(result.children).toHaveLength(3)
  })

  it("renders connections as paths", () => {
    const result = renderLayout(simpleLayout)
    const path = result.children[0] as SVGPath
    
    expect(path.type).toBe("path")
    expect(path.d).toContain("M")
    expect(path.className).toBe("flow-connection")
    expect(path.dataParentId).toBe("root")
    expect(path.dataChildId).toBe("child")
  })

  it("renders node groups with rect and text", () => {
    const result = renderLayout(simpleLayout)
    const nodeGroup = result.children[1] as SVGGroup
    
    expect(nodeGroup.type).toBe("g")
    expect(nodeGroup.className).toBe("flow-node-group")
    expect(nodeGroup.children).toHaveLength(2)
    
    const rect = nodeGroup.children[0] as SVGRect
    expect(rect.type).toBe("rect")
    expect(rect.x).toBe(0)
    expect(rect.y).toBe(0)
    expect(rect.width).toBe(100)
    expect(rect.height).toBe(50)
    expect(rect.dataNodeId).toBe("root")
    
    const text = nodeGroup.children[1] as SVGText
    expect(text.type).toBe("text")
    expect(text.text).toBe("Root Node")
    expect(text.x).toBe(50) // centered
    expect(text.y).toBe(25) // centered
  })

  it("applies status colors to nodes", () => {
    const layoutWithStatus: LayoutOutput = {
      nodes: [
        {
          id: "busy-node",
          type: "task",
          label: "Busy",
          x: 0,
          y: 0,
          size: { width: 100, height: 50 },
          metadata: { status: "busy" },
        } as PositionedNode,
      ],
      connections: [],
    }
    
    const result = renderLayout(layoutWithStatus)
    const nodeGroup = result.children[0] as SVGGroup
    const rect = nodeGroup.children[0] as SVGRect
    
    expect(rect.fill).toBe(DEFAULT_RENDER_CONFIG.statusColors.busy)
  })

  it("works with sample MechaCoder tree", () => {
    const layout = calculateLayout({
      root: sampleMechaCoderTree,
      nodeSizes: sampleNodeSizes,
      config: { padding: 8, spacing: 8 },
    })
    
    const result = renderLayout(layout)
    
    expect(result.type).toBe("g")
    expect(result.children.length).toBeGreaterThan(0)
    
    // Should have nodes for all items in sample tree
    const nodeGroups = result.children.filter(c => c.type === "g") as SVGGroup[]
    expect(nodeGroups.length).toBeGreaterThan(5)
  })
})

describe("renderFlowSVG", () => {
  const simpleLayout: LayoutOutput = {
    nodes: [
      {
        id: "node",
        type: "task",
        label: "Node",
        x: 0,
        y: 0,
        size: { width: 100, height: 50 },
      } as PositionedNode,
    ],
    connections: [],
  }

  it("applies canvas transform", () => {
    const canvas: CanvasState = {
      ...initialCanvasState(800, 600),
      scale: 2,
      panX: 100,
      panY: 50,
    }
    
    const result = renderFlowSVG(simpleLayout, canvas)
    
    expect(result.type).toBe("g")
    expect(result.transform).toBe("translate(100, 50) scale(2)")
    expect(result.className).toBe("flow-canvas")
  })

  it("nests content inside transform group", () => {
    const canvas = initialCanvasState(800, 600)
    const result = renderFlowSVG(simpleLayout, canvas)
    
    expect(result.children).toHaveLength(1)
    expect((result.children[0] as SVGGroup).className).toBe("flow-content")
  })
})

describe("getCanvasTransform", () => {
  it("returns translate + scale transform", () => {
    const canvas: CanvasState = {
      ...initialCanvasState(800, 600),
      scale: 1.5,
      panX: 200,
      panY: -100,
    }
    
    const transform = getCanvasTransform(canvas)
    
    expect(transform).toBe("translate(200, -100) scale(1.5)")
  })

  it("handles default state", () => {
    const canvas = initialCanvasState(800, 600)
    const transform = getCanvasTransform(canvas)
    
    expect(transform).toBe("translate(0, 0) scale(1)")
  })
})

describe("svgElementToString", () => {
  it("converts rect to SVG string", () => {
    const rect: SVGRect = {
      type: "rect",
      x: 10,
      y: 20,
      width: 100,
      height: 50,
      rx: 5,
      fill: "#fff",
      className: "test-rect",
    }
    
    const str = svgElementToString(rect)
    
    expect(str).toContain('<rect')
    expect(str).toContain('x="10"')
    expect(str).toContain('y="20"')
    expect(str).toContain('width="100"')
    expect(str).toContain('height="50"')
    expect(str).toContain('rx="5"')
    expect(str).toContain('fill="#fff"')
    expect(str).toContain('class="test-rect"')
  })

  it("converts text to SVG string with escaping", () => {
    const text: SVGText = {
      type: "text",
      x: 50,
      y: 25,
      text: "Hello <World> & \"Friends\"",
      fill: "#000",
    }
    
    const str = svgElementToString(text)
    
    expect(str).toContain('<text')
    expect(str).toContain('x="50"')
    expect(str).toContain('Hello &lt;World&gt; &amp; &quot;Friends&quot;')
    expect(str).toContain('</text>')
  })

  it("converts path to SVG string", () => {
    const path: SVGPath = {
      type: "path",
      d: "M 0 0 L 100 100",
      stroke: "#666",
      fill: "none",
    }
    
    const str = svgElementToString(path)
    
    expect(str).toContain('<path')
    expect(str).toContain('d="M 0 0 L 100 100"')
    expect(str).toContain('stroke="#666"')
    expect(str).toContain('fill="none"')
  })

  it("converts group with children", () => {
    const group: SVGGroup = {
      type: "g",
      transform: "translate(10, 20)",
      className: "test-group",
      children: [
        { type: "rect", x: 0, y: 0, width: 10, height: 10 },
      ],
    }
    
    const str = svgElementToString(group)
    
    expect(str).toContain('<g')
    expect(str).toContain('transform="translate(10, 20)"')
    expect(str).toContain('class="test-group"')
    expect(str).toContain('<rect')
    expect(str).toContain('</g>')
  })
})

describe("renderToSVGString", () => {
  it("generates complete SVG document", () => {
    const layout: LayoutOutput = {
      nodes: [
        {
          id: "node",
          type: "task",
          label: "Test",
          x: 0,
          y: 0,
          size: { width: 100, height: 50 },
        } as PositionedNode,
      ],
      connections: [],
    }
    const canvas = initialCanvasState(800, 600)
    
    const svg = renderToSVGString(layout, canvas)
    
    expect(svg).toContain('<svg')
    expect(svg).toContain('xmlns="http://www.w3.org/2000/svg"')
    expect(svg).toContain('width="800"')
    expect(svg).toContain('height="600"')
    expect(svg).toContain('viewBox="0 0 800 600"')
    expect(svg).toContain('</svg>')
    expect(svg).toContain('<g')
    expect(svg).toContain('<rect')
    expect(svg).toContain('<text')
  })

  it("works with sample data end-to-end", () => {
    const layout = calculateLayout({
      root: sampleMechaCoderTree,
      nodeSizes: sampleNodeSizes,
      config: { padding: 8, spacing: 8 },
    })
    const canvas = initialCanvasState(1200, 800)
    
    const svg = renderToSVGString(layout, canvas)
    
    expect(svg).toContain('<svg')
    expect(svg).toContain('OpenAgents Desktop')
    expect(svg).toContain('MechaCoder Agent')
    expect(svg).toContain('Repo: openagents')
    expect(svg).toContain('</svg>')
  })
})
