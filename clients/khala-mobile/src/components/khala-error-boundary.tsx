import { Component, type ErrorInfo, type ReactNode } from "react"

import { tx } from "../i18n/copy"
import { KhalaButton } from "./khala-button"
import { KhalaScreen } from "./khala-screen"
import { KhalaText } from "./khala-text"

type KhalaErrorBoundaryProps = Readonly<{
  children: ReactNode
}>

type KhalaErrorBoundaryState = Readonly<{
  error: Error | null
}>

const KhalaErrorFallback = ({ onReset }: { onReset: () => void }) => (
  <KhalaScreen contentClassName="items-center justify-center px-6" preset="fixed">
    <KhalaText className="text-center" text={tx("app.title")} variant="heading" />
    <KhalaText
      className="mt-4 text-center"
      text={tx("errorBoundary.body")}
      variant="muted"
    />
    <KhalaText
      className="mt-2 text-center"
      text={tx("errorBoundary.help")}
      variant="faint"
    />
    <KhalaButton className="mt-8 self-stretch" onPress={onReset} text={tx("errorBoundary.retry")} variant="primary" />
  </KhalaScreen>
)

export class KhalaErrorBoundary extends Component<
  KhalaErrorBoundaryProps,
  KhalaErrorBoundaryState
> {
  state: KhalaErrorBoundaryState = { error: null }

  static getDerivedStateFromError(error: Error): KhalaErrorBoundaryState {
    return { error }
  }

  componentDidCatch(_error: Error, _errorInfo: ErrorInfo) {
    console.error("Khala mobile render error boundary tripped")
  }

  reset = () => {
    this.setState({ error: null })
  }

  render() {
    if (this.state.error !== null) return <KhalaErrorFallback onReset={this.reset} />
    return this.props.children
  }
}
