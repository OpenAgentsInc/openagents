import { Control } from "./Control"
import { StyledContent, StyledWrapper } from "./components/Folder/StyledFolder"

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
    <StyledWrapper>
      <StyledContent>
        {entries.map(([key, value]) => (
          // @ts-ignore
          <Control key={key} path={value.path} data={value.data} />
        ))}
      </StyledContent>
    </StyledWrapper>
  )
}
