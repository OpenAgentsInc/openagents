import { Control } from "./Control"
import { StyledContent, StyledWrapper } from "./components/Folder/StyledFolder"

export type Tree = {
  [key: string]: { path: string, data: any } | Tree
}

type TreeWrapperProps = {
  toggled: boolean
  tree: Tree
}

export const TreeWrapper = ({ toggled, tree }: TreeWrapperProps) => {
  const entries = Object.entries(tree)
  // console.log("TreeWrapper with entries:", entries)
  return (
    <StyledWrapper>
      <StyledContent toggled={toggled}>
        {entries.map(([key, value]) => (
          // @ts-ignore
          <Control key={key} path={value.path} data={value.data} />
        ))}
      </StyledContent>
    </StyledWrapper>
  )
}
