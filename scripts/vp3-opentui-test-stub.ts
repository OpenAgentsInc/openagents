class Renderable {
  content = ""
  children: unknown[] = []
  constructor(..._args: unknown[]) {}
  add(child: unknown) { this.children.push(child); return child }
  destroy() {}
  remove(child: unknown) { this.children = this.children.filter(value => value !== child) }
}

export class MarkdownRenderable extends Renderable {}
export class CodeRenderable extends Renderable {}
export class DiffRenderable extends Renderable {}
export class BoxRenderable extends Renderable {}
export class TextRenderable extends Renderable {}
export class ScrollBoxRenderable extends Renderable {}
export class LineNumberRenderable extends Renderable {}
export class SyntaxStyle { static fromStyles(styles: unknown) { return new SyntaxStyle(styles) } constructor(readonly styles: unknown) {} }
export const parseColor = (color: string) => color
export const createCliRenderer = async () => ({
  destroy() {},
  requestRender() {},
  root: new Renderable(),
  start() {},
  stop() {},
})
