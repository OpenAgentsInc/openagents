import { Control } from "./Control"

export type Tree = {
  [key: string]: { path: string, data: any } | Tree
}

type TreeWrapperProps = {
  tree: Tree
}

export const TreeWrapper = ({ tree }: TreeWrapperProps) => {
  const entries = Object.entries(tree)
  // console.log("TreeWrapper with entries:", entries)
  return (
    <>
      {entries.map(([key, value]) => (
        // @ts-ignore
        <Control key={key} path={value.path} data={value.data} />
      ))}
    </>
  )
}
