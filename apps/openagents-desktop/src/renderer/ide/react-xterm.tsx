import "./react-run.css"

import type { ReactElement } from "react"
import { useEffect, useRef, useState } from "react"

import { FitAddon } from "@xterm/addon-fit"
import { SearchAddon } from "@xterm/addon-search"
import { SerializeAddon } from "@xterm/addon-serialize"
import { WebLinksAddon } from "@xterm/addon-web-links"
import { Terminal as XtermTerminal } from "@xterm/xterm"
import { ChevronDown, ChevronUp, Search, X } from "lucide-react"

import { Button } from "../../components/ui/button.tsx"
import { Input } from "../../components/ui/input.tsx"
import type { TerminalRendererSession } from "../terminal-workspace.ts"

export type ReactXtermProjectionProps = Readonly<{
  session: TerminalRendererSession
  onInput: (data: string) => void
  onResize: (cols: number, rows: number) => void
  onOpenPreview: (port: number) => void
}>

const previewPort = (session: TerminalRendererSession, uri: string): number | null => {
  try {
    const url = new URL(uri)
    if (url.protocol !== "http:" && url.protocol !== "https:") return null
    if (url.hostname !== "localhost" && url.hostname !== "127.0.0.1" && url.hostname !== "[::1]") return null
    const port = Number.parseInt(url.port, 10)
    if (!Number.isInteger(port)) return null
    return session.previews.some((preview) => preview.port === port && preview.ready) ? port : null
  } catch {
    return null
  }
}

export const ReactXtermProjection = ({
  session,
  onInput,
  onResize,
  onOpenPreview,
}: ReactXtermProjectionProps): ReactElement => {
  const containerRef = useRef<HTMLDivElement>(null)
  const terminalRef = useRef<XtermTerminal | null>(null)
  const searchRef = useRef<SearchAddon | null>(null)
  const serializeRef = useRef<SerializeAddon | null>(null)
  const lastOutputRef = useRef("")
  const [searchOpen, setSearchOpen] = useState(false)
  const [query, setQuery] = useState("")

  useEffect(() => {
    const container = containerRef.current
    if (container === null) return
    const terminal = new XtermTerminal({
      allowProposedApi: false,
      convertEol: true,
      cursorBlink: false,
      cursorStyle: "block",
      fontFamily: "var(--oa-font-mono)",
      fontSize: 12,
      lineHeight: 1.35,
      minimumContrastRatio: 4.5,
      rightClickSelectsWord: true,
      screenReaderMode: true,
      scrollback: 10_000,
      theme: {
        background: "#05070d",
        foreground: "#c0caf5",
        cursor: "#3b82f6",
        cursorAccent: "#05070d",
        selectionBackground: "#283457",
        black: "#15161e",
        red: "#f7768e",
        green: "#9ece6a",
        yellow: "#e0af68",
        blue: "#3b82f6",
        magenta: "#bb9af7",
        cyan: "#7dcfff",
        white: "#a9b1d6",
        brightBlack: "#414868",
        brightRed: "#f7768e",
        brightGreen: "#9ece6a",
        brightYellow: "#e0af68",
        brightBlue: "#7aa2f7",
        brightMagenta: "#bb9af7",
        brightCyan: "#7dcfff",
        brightWhite: "#c0caf5",
      },
    })
    const fit = new FitAddon()
    const search = new SearchAddon()
    const serialize = new SerializeAddon()
    terminal.loadAddon(fit)
    terminal.loadAddon(search)
    terminal.loadAddon(serialize)
    terminal.loadAddon(new WebLinksAddon((event, uri) => {
      event.preventDefault()
      const port = previewPort(session, uri)
      if (port !== null) onOpenPreview(port)
    }))
    terminal.open(container)
    terminalRef.current = terminal
    searchRef.current = search
    serializeRef.current = serialize
    lastOutputRef.current = session.output
    terminal.write(session.output)
    const input = terminal.onData((data) => onInput(data))
    let lastGeometry = ""
    const fitAndPublish = (): void => {
      try { fit.fit() } catch { return }
      const geometry = `${terminal.cols}:${terminal.rows}`
      if (geometry === lastGeometry) return
      lastGeometry = geometry
      onResize(terminal.cols, terminal.rows)
    }
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(fitAndPublish)
    observer?.observe(container)
    const frame = requestAnimationFrame(fitAndPublish)
    return () => {
      cancelAnimationFrame(frame)
      observer?.disconnect()
      input.dispose()
      terminal.dispose()
      terminalRef.current = null
      searchRef.current = null
      serializeRef.current = null
      lastOutputRef.current = ""
    }
  }, [onInput, onOpenPreview, onResize, session.sessionRef])

  useEffect(() => {
    const terminal = terminalRef.current
    if (terminal === null) return
    const previous = lastOutputRef.current
    if (session.output.startsWith(previous)) {
      const delta = session.output.slice(previous.length)
      if (delta !== "") terminal.write(delta)
    } else {
      terminal.reset()
      terminal.write(session.output)
    }
    lastOutputRef.current = session.output
    containerRef.current?.setAttribute("data-serialized-screen-bytes", String(serializeRef.current?.serialize().length ?? 0))
  }, [session.output])

  const search = (direction: "next" | "previous"): void => {
    if (query === "") return
    if (direction === "next") searchRef.current?.findNext(query, { incremental: true })
    else searchRef.current?.findPrevious(query, { incremental: true })
  }

  return <div className="oa-react-xterm-shell" data-xterm-projection="true" data-xterm-search="true" data-xterm-serialize="true" data-xterm-web-links="policy-bound">
    <div className="oa-react-xterm-find" hidden={!searchOpen}>
      <Input aria-label="Search terminal output" value={query} onChange={(event) => { setQuery(event.currentTarget.value); searchRef.current?.findNext(event.currentTarget.value, { incremental: true }) }} onKeyDown={(event) => { if (event.key === "Enter") search(event.shiftKey ? "previous" : "next"); if (event.key === "Escape") { setSearchOpen(false); terminalRef.current?.focus() } }} />
      <Button size="icon-sm" variant="ghost" aria-label="Previous terminal match" onClick={() => search("previous")}><ChevronUp aria-hidden="true" /></Button>
      <Button size="icon-sm" variant="ghost" aria-label="Next terminal match" onClick={() => search("next")}><ChevronDown aria-hidden="true" /></Button>
      <Button size="icon-sm" variant="ghost" aria-label="Close terminal search" onClick={() => { setSearchOpen(false); terminalRef.current?.focus() }}><X aria-hidden="true" /></Button>
    </div>
    <Button className="oa-react-xterm-search-toggle" size="icon-sm" variant="ghost" aria-label="Search terminal" aria-expanded={searchOpen} onClick={() => setSearchOpen((open) => !open)}><Search aria-hidden="true" /></Button>
    <div className="oa-react-xterm" ref={containerRef} role="application" aria-label={`Interactive terminal ${session.shellLabel}`} />
  </div>
}
