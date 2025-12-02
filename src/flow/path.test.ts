import { describe, it, expect } from 'bun:test'
import { buildRoundedPath, type PathConfig } from './path.js'
import type { Point } from './model.js'

describe('buildRoundedPath', () => {
  const config: PathConfig = { cornerRadius: 8 }

  it('straight horizontal (2 points)', () => {
    const points: Point[] = [{ x: 0, y: 0 }, { x: 100, y: 0 }]
    const path = buildRoundedPath(points, config)
    expect(path).toBe('M 0.00 0.00 L 100.00 0.00')
  })

  it('straight vertical', () => {
    const points: Point[] = [{ x: 0, y: 0 }, { x: 0, y: 100 }]
    const path = buildRoundedPath(points, config)
    expect(path).toBe('M 0.00 0.00 L 0.00 100.00')
  })

  it('diagonal straight', () => {
    const points: Point[] = [{ x: 0, y: 0 }, { x: 100, y: 50 }]
    const path = buildRoundedPath(points, config)
    expect(path).toBe('M 0.00 0.00 L 100.00 50.00')
  })

  it('L-shape: right then down (90° right turn)', () => {
    const points: Point[] = [
      { x: 0, y: 0 },
      { x: 100, y: 0 },
      { x: 100, y: 100 }
    ]
    const path = buildRoundedPath(points, config)
    expect(path).toBe(
      'M 0.00 0.00 L 92.00 0.00 A 8.00 8.00 0 0 1 100.00 8.00 L 100.00 100.00'
    )
  })

  it('L-shape: right then up (90° left turn)', () => {
    const points: Point[] = [
      { x: 0, y: 0 },
      { x: 100, y: 0 },
      { x: 100, y: -100 }
    ]
    const path = buildRoundedPath(points, config)
    expect(path).toBe(
      'M 0.00 0.00 L 92.00 0.00 A 8.00 8.00 0 0 0 100.00 -8.00 L 100.00 -100.00'
    )
  })

  it('U-shape: right-down-right (two corners)', () => {
    const points: Point[] = [
      { x: 0, y: 0 },
      { x: 100, y: 0 },
      { x: 100, y: 100 },
      { x: 200, y: 100 }
    ]
    const path = buildRoundedPath(points, config)
    // After first arc ends at (100,8), line goes to corner approach at (100,92), then arcs to (108,100)
    expect(path).toBe(
      'M 0.00 0.00 L 92.00 0.00 A 8.00 8.00 0 0 1 100.00 8.00 L 100.00 92.00 A 8.00 8.00 0 0 0 108.00 100.00 L 200.00 100.00'
    )
  })

  it('short segment skips rounding', () => {
    const points: Point[] = [
      { x: 0, y: 0 },
      { x: 10, y: 0 }, // len1=10 <16=2*8
      { x: 10, y: 100 }
    ]
    const path = buildRoundedPath(points, config)
    expect(path).toBe('M 0.00 0.00 L 10.00 0.00 L 10.00 100.00')
  })

  it('collinear skips rounding', () => {
    const points: Point[] = [
      { x: 0, y: 0 },
      { x: 100, y: 0 },
      { x: 200, y: 0 }
    ]
    const path = buildRoundedPath(points, config)
    expect(path).toBe('M 0.00 0.00 L 100.00 0.00 L 200.00 0.00')
  })

  it('zero radius: straight polyline', () => {
    const points: Point[] = [
      { x: 0, y: 0 },
      { x: 100, y: 0 },
      { x: 100, y: 100 }
    ]
    const path = buildRoundedPath(points, { cornerRadius: 0 })
    expect(path).toBe('M 0.00 0.00 L 100.00 0.00 L 100.00 100.00')
  })

  it('r=0 on complex', () => {
    const points: Point[] = [{ x: 0, y: 0 }, { x: 100, y: 50 }]
    const path = buildRoundedPath(points, { cornerRadius: 0 })
    expect(path).toBe('M 0.00 0.00 L 100.00 50.00')
  })
})
