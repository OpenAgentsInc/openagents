// A build's generated file, written only when its bytes change: an unchanged
// output leaves a tracked file alone (no dirty checkout) and doesn't wake coax
// serve's watcher. volatile matches lines that alone don't make a change (a
// capture time): when only they differ, the file keeps its old ones too.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs"
import { dirname } from "node:path"

export function writeChanged(file: string, text: string, volatile?: RegExp): boolean {
  if (existsSync(file)) {
    const old = readFileSync(file, "utf8")
    if (old === text) return false
    const keep = (s: string) => s.split("\n").filter((l) => !volatile?.test(l)).join("\n")
    if (volatile && keep(old) === keep(text)) return false
  }
  mkdirSync(dirname(file), { recursive: true })
  writeFileSync(file, text)
  return true
}
