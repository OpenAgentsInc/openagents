import { Step } from "@/types/agents"
import { Tree } from "./TreeWrapper"

export const buildTree = (data: Step) => {
  // for now just put each field of the Step in its own field in the Tree
  const tree: Tree = {}
  Object.keys(data).forEach(key => {
    tree[key] = { path: key, data: data[key] }
  })
  return tree
}
