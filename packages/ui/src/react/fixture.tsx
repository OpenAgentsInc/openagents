import {
  AnchorButton,
  Button,
  Card,
  CardDescription,
  CardHeader,
  CardTitle,
  CodeBlock,
  Panel,
  PanelHeader,
  TextareaField,
  TextField,
  TopNav,
} from './components'

export const ReactEditionSmokeFixture = (): React.JSX.Element => (
  <div className="oa-react-ui-root min-h-screen bg-oa-bg text-oa-text-body">
    <TopNav
      actions={<AnchorButton href="#deploy">Deploy notes</AnchorButton>}
      brand="OPENAGENTS / START"
      items={[
        { current: true, href: '#funnel', label: 'Funnel' },
        { href: '#sync', label: 'Sync' },
        { href: '#ops', label: 'Ops' },
      ]}
    />
    <main className="mx-auto grid w-full max-w-6xl gap-6 px-4 py-6">
      <Panel aria-labelledby="funnel-heading" id="funnel">
        <PanelHeader>
          <div>
            <p className="font-oa-mono text-sm text-oa-accent">staging scaffold</p>
            <h1
              className="max-w-[18ch] text-3xl font-medium tracking-normal text-oa-text sm:text-4xl"
              id="funnel-heading"
            >
              Turn agent work into visible proof.
            </h1>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button>Start review</Button>
            <Button variant="ghost">Inspect trace</Button>
          </div>
        </PanelHeader>
        <div className="grid gap-4 lg:grid-cols-3">
          <Card>
            <CardHeader>
              <CardTitle>Worker-safe</CardTitle>
              <CardDescription>
                Components consume the same dark tokens as the existing Worker
                pages without adding light-mode machinery.
              </CardDescription>
            </CardHeader>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>Native-ready</CardTitle>
              <CardDescription>
                The exported NativeWind token object carries literal values for
                the Expo companion track.
              </CardDescription>
            </CardHeader>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>Funnel-first</CardTitle>
              <CardDescription>
                Buttons, panels, cards, nav, forms, and code blocks cover the
                first public Start pages.
              </CardDescription>
            </CardHeader>
          </Card>
        </div>
      </Panel>

      <section className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_minmax(320px,420px)]">
        <Panel aria-labelledby="form-heading">
          <PanelHeader>
            <h2 className="text-xl font-medium tracking-normal text-oa-text" id="form-heading">
              Intake controls
            </h2>
          </PanelHeader>
          <form className="grid gap-4">
            <TextField
              description="Dark-only field chrome with mobile-sized body text."
              label="Workspace"
              name="workspace"
              placeholder="openagents.com"
            />
            <TextareaField
              label="Request"
              name="request"
              placeholder="Summarize the staging deploy evidence."
            />
            <div className="flex justify-end gap-2">
              <Button variant="secondary">Save draft</Button>
              <Button type="submit">Queue worker</Button>
            </div>
          </form>
        </Panel>

        <CodeBlock
          code={'import "@openagentsinc/ui/react.css"\\nimport { Button } from "@openagentsinc/ui/react"'}
          filename="src/routes/index.tsx"
          language="tsx"
        />
      </section>
    </main>
  </div>
)
