import { Context, Effect, Layer } from "effect"
import * as NodeFs from "node:fs"

// Define the capabilities we need from the filesystem
export interface FileSystem {
  readonly existsSync: (path: string) => boolean
  readonly mkdirSync: (path: string, options?: { recursive?: boolean }) => void
  readonly writeFileSync: (path: string, data: string) => void
  readonly readFileSync: (path: string, encoding: BufferEncoding) => string
}

// Create a context for FileSystem
export const FileSystem = Context.Tag<FileSystem>("FileSystem")

// Live implementation of FileSystem using Node's fs module
export const FileSystemLive = Layer.succeed(
  FileSystem,
  {
    existsSync: (path: string): boolean => NodeFs.existsSync(path),
    mkdirSync: (path: string, options?: { recursive?: boolean }): void => NodeFs.mkdirSync(path, options),
    writeFileSync: (path: string, data: string): void => NodeFs.writeFileSync(path, data),
    readFileSync: (path: string, encoding: BufferEncoding): string => NodeFs.readFileSync(path, encoding)
  }
)