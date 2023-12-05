import { RunTable } from '@/Components/RunTable'
import { Button } from '@/Components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/Components/ui/card'
import InspectLayout from '@/Layouts/InspectLayout'
import { EnvelopeOpenIcon, GitHubLogoIcon } from '@radix-ui/react-icons'

function Login() {
    return (
        <div className="absolute h-[80vh] w-screen flex flex-col justify-center items-center -pt-12 px-4 w-auto">
            <Card>
                <CardHeader>
                    <CardTitle>Welcome!</CardTitle>
                    <CardDescription>Please log in to continue.</CardDescription>
                </CardHeader>
                <CardContent className="text-center">
                    <Button size="lg" className="mt-2 px-4 py-6">
                        <GitHubLogoIcon className="mr-3 h-4 w-4" />
                        <span className="text-lg">Log in with GitHub</span>
                    </Button>
                    <p className="text-muted-foreground mt-8 text-xs">By logging in, you agree to our terms of service and privacy policy.</p>
                </CardContent>
            </Card>
        </div>
    )
}

Login.layout = (page) => <InspectLayout children={page} title="Login" />

export default Login
