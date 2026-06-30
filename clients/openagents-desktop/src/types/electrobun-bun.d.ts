declare module "electrobun/bun" {
  export class BrowserWindow {
    constructor(options: {
      readonly title: string
      readonly url: string
      readonly frame?: {
        readonly x?: number
        readonly y?: number
        readonly width?: number
        readonly height?: number
      }
      readonly rpc?: unknown
    })
  }
}
