/**
 * OpenClaw chat UI – same structure and styling as ~/code/openclaw chat view.
 * No functionality wired; screen components only.
 */
import '@/styles/openclaw-ui.css';

export function OpenClawChatUI() {
  return (
    <div className="openclaw-ui h-full min-h-0 flex flex-col">
      <div className="oc-shell flex-1 min-h-0 flex flex-col">
        <header className="oc-topbar">
          <div className="oc-topbar-left">
            <div className="oc-brand">
              <div className="oc-brand-logo">
                <img src="/favicon.svg" alt="OpenClaw" />
              </div>
              <div className="oc-brand-text">
                <div className="oc-brand-title">OPENCLAW</div>
                <div className="oc-brand-sub">Gateway Dashboard</div>
              </div>
            </div>
          </div>
          <div className="oc-topbar-status">
            <div className="oc-pill">
              <span className="oc-status-dot ok" />
              <span>Health</span>
              <span className="font-mono text-xs ml-1">OK</span>
            </div>
          </div>
        </header>

        <main className="oc-content flex-1 min-h-0 flex flex-col">
          <section className="oc-content-header">
            <div>
              <div className="oc-page-title">Chat</div>
              <div className="oc-page-sub">Sessions and streaming</div>
            </div>
          </section>

          <section className="oc-chat flex-1 min-h-0 flex flex-col p-4">
            <div className="oc-chat-thread" role="log" aria-live="polite">
              <div className="oc-muted py-4">No messages yet. Connect to the gateway to start chatting.</div>
            </div>

            <div className="oc-chat-compose">
              <div className="oc-chat-compose__row">
                <label className="oc-chat-compose__field flex flex-col gap-1.5">
                  <span>Message</span>
                  <textarea
                    placeholder="Message (↩ to send, Shift+↩ for line breaks, paste images)"
                    disabled
                    readOnly
                    className="rounded-lg"
                  />
                </label>
                <div className="oc-chat-compose__actions">
                  <button type="button" className="oc-btn" disabled>
                    New session
                  </button>
                  <button type="button" className="oc-btn primary" disabled>
                    Send <kbd className="ml-1.5 rounded px-1 py-0.5 text-[11px] bg-white/15">↵</kbd>
                  </button>
                </div>
              </div>
            </div>
          </section>
        </main>
      </div>
    </div>
  );
}
