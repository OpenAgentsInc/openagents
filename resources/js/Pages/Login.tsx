import NavLink from '@/Components/NavLink'
import { Alert, AlertDescription, AlertTitle } from '@/Components/ui/alert'
import { Button } from '@/Components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/Components/ui/card'
import InspectLayout from '@/Layouts/InspectLayout'
import { XCircleIcon } from '@heroicons/react/24/outline'
import { Link } from '@inertiajs/react'
import { GitHubLogoIcon, RocketIcon } from '@radix-ui/react-icons'

function Login() {
  return (
    <div className="absolute h-[80vh] w-screen flex flex-col items-center pt-16 sm:pt-32 px-4 w-auto">
      <Card className="mb-8">
        <Alert variant="destructive">
          <XCircleIcon className="h-6 w-6 -mt-1" />
          <AlertTitle className="ml-1 font-bold">Developer preview</AlertTitle>
          <AlertDescription className="ml-1">
            This is pre-release code auto-deployed from our <a className="font-medium underline" target="_blank" href="https://github.com/OpenAgentsInc/openagents">GitHub repo</a>.<br />Don't expect anything to work!
          </AlertDescription>
        </Alert>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle>Welcome to OpenAgents!</CardTitle>
          <CardDescription>Please log in to continue.</CardDescription>
        </CardHeader>
        <CardContent className="text-center">
          <a href="/login/github">
            <Button size="lg" className="mt-2 px-4 py-6">
              <GitHubLogoIcon className="mr-3 h-6 w-6" />
              <span className="text-lg">Log in with GitHub</span>
            </Button>
          </a>
          <p className="text-muted-foreground mt-8 text-xs">By logging in you agree to our <Link href="/terms" className="underline">terms of service</Link> and <Link href="/privacy" className="underline">privacy policy</Link>.</p>
        </CardContent>
      </Card>
    </div>
  )
}

Login.layout = (page) => <InspectLayout children={page} title="Login" />

export default Login
