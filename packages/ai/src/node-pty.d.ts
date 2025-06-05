declare module "node-pty" {
  export interface IPtyForkOptions {
    name?: string
    cols?: number
    rows?: number
    cwd?: string
    env?: { [key: string]: string | undefined }
    encoding?: string
    handleFlowControl?: boolean
    flowControlPause?: string
    flowControlResume?: string
  }

  export interface IPty {
    onData: (callback: (data: string) => void) => void
    onExit: (callback: (exitCode: { exitCode: number; signal?: number }) => void) => void
    write: (data: string) => void
    resize: (cols: number, rows: number) => void
    kill: (signal?: string) => void
    pid: number
  }

  export function spawn(
    file: string,
    args: Array<string>,
    options: IPtyForkOptions
  ): IPty
}
