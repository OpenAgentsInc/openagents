declare module 'react-native-syntax-highlighter' {
  import { StyleProp, TextStyle } from 'react-native'

  interface SyntaxHighlighterProps {
    language: string
    style?: any
    customStyle?: StyleProp<TextStyle>
    fontSize?: number
    fontFamily?: string
    children: string
    highlighter?: 'prism' | 'hljs'
  }

  export default function SyntaxHighlighter(props: SyntaxHighlighterProps): JSX.Element
}

declare module 'react-syntax-highlighter/dist/esm/styles/prism' {
  export const atomDark: any
}

declare module 'react-syntax-highlighter/dist/esm/styles/hljs' {
  export const vs2015: any
}
