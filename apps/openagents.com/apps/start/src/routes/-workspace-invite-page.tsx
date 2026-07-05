const eyebrowClass =
  'm-0 font-mono text-[0.6875rem] uppercase text-khala-text-faint'

const bodyClass = 'm-0 text-base/7 text-khala-text-muted'

export function WorkspaceInvitePage({
  workspaceId,
}: Readonly<{ workspaceId: string }>) {
  return (
    <main
      className="mx-auto grid min-h-[70svh] w-[min(100%,720px)] content-center gap-5 px-4 py-10 text-khala-text"
      data-route="workspace-invite"
    >
      <div className="grid gap-3 border border-khala-border bg-khala-void p-5 sm:p-6">
        <p className={eyebrowClass}>Workspace invite</p>
        <h1 className="m-0 text-2xl font-medium tracking-normal text-khala-text sm:text-3xl">
          Open your project workspace
        </h1>
        <p className={bodyClass}>
          Your project setup is waiting. Sign in to review the seeded notes
          and starter workflows.
        </p>
        <p className="m-0 font-mono text-xs text-khala-text-faint">
          {workspaceId}
        </p>
        <a
          className="khala-focus inline-flex min-h-10 w-fit items-center border border-khala-text bg-khala-text px-4 font-mono text-[0.8125rem] text-black hover:bg-white"
          href="/login/github"
        >
          Log in with GitHub
        </a>
      </div>
    </main>
  )
}
