import { useEffect, useRef, type ReactElement, type ReactNode } from "react"

export const DesktopTimeline = ({ children, followKey, working = false }: Readonly<{
  children: ReactNode
  followKey?: string | number
  working?: boolean
}>): ReactElement => {
  const viewport = useRef<HTMLDivElement>(null)
  const readerAtLatest = useRef(true)

  useEffect(() => {
    const element = viewport.current
    if (element === null || !readerAtLatest.current) return
    element.scrollTop = element.scrollHeight
  }, [followKey])

  return <section aria-label="Conversation timeline" className="oa-react-timeline-region">
    <div
      className="oa-react-timeline-scroll"
      onScroll={event => {
        const element = event.currentTarget
        readerAtLatest.current = element.scrollHeight - element.scrollTop - element.clientHeight < 80
      }}
      ref={viewport}
    >
      {/* Quantized block-progress motif (design spec §5.2): discrete steps,
          never a smooth/eased fill — see .oa-react-working in desktop-workbench.css. */}
      <div aria-busy={working} className="oa-react-timeline-content" role="list">
        {children}
        {working ? <div className="oa-react-working" role="status" aria-label="Codex is working"><span>Working</span><i /><i /><i /></div> : null}
      </div>
    </div>
  </section>
}
