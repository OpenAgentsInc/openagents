import { StyledRoot } from "./components/Agentgraph/StyledRoot"
import { ThemeContext } from "./context"
import { useDeepMemo } from "./hooks/useDeepMemo"
import { getDefaultTheme, mergeTheme } from "./styles"

export const Canvas = ({ children }: { children: any }) => {
  const theme = getDefaultTheme()
  const themeContext = useDeepMemo(() => mergeTheme(theme), [theme])
  return (
    <div className="w-full h-full">
      <ThemeContext.Provider value={themeContext}>
        <StyledRoot>
          {children}
        </StyledRoot>
      </ThemeContext.Provider>
    </div>
  )
}
