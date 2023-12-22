import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/Components/ui/card'
import InspectLayout from '@/Layouts/InspectLayout'

function Terms() {
  return (
    <div className="min-h-screen w-screen flex flex-col justify-center items-center -pt-12 px-4 w-auto">
      <Card className="px-4 my-12 w-full sm:w-2/3">
        <CardHeader>
          <CardTitle>Terms of service</CardTitle>
          <CardDescription>Last updated December 22, 2023</CardDescription>
        </CardHeader>
        <CardContent className="text-muted-foreground space-y-4 -mt-2 text-sm">
          <div className="container mx-auto px-4">
            <h2 className="text-xl font-semibold mt-6">1. Acceptance of Terms</h2>
            <p className="mt-2">By accessing or using the services provided by OpenAgents, Inc. ("OpenAgents"), you agree to be bound by these Terms of Service ("Terms"). If you do not agree to these Terms, you may not use our services.</p>

            <h2 className="text-xl font-semibold mt-6">2. Description of Service</h2>
            <p className="mt-2">OpenAgents provides a marketplace for AI agents, offering various tools and resources for users to access and interact with AI technologies. The services may be updated or modified from time to time at OpenAgents' discretion.</p>

            <h2 className="text-xl font-semibold mt-6">3. User Responsibilities</h2>
            <p className="mt-2">Users are responsible for all activities under their account. You agree to use the services legally and ethically, and not to violate any applicable laws or regulations.</p>

            <h2 className="text-xl font-semibold mt-6">4. Intellectual Property Rights</h2>
            <p className="mt-2">All content provided on OpenAgents, including but not limited to text, graphics, logos, and software, is the property of OpenAgents or its licensors and is protected by United States and international intellectual property laws.</p>

            <h2 className="text-xl font-semibold mt-6">5. Privacy Policy</h2>
            <p className="mt-2">Your use of OpenAgents is also governed by our Privacy Policy, which is incorporated into these Terms by reference.</p>

            <h2 className="text-xl font-semibold mt-6">6. Limitation of Liability</h2>
            <p className="mt-2">OpenAgents shall not be liable for any indirect, incidental, special, consequential, or punitive damages arising out of or related to your use of the services.</p>

            <h2 className="text-xl font-semibold mt-6">7. Indemnification</h2>
            <p className="mt-2">You agree to indemnify and hold harmless OpenAgents and its officers, directors, employees, and agents from any claims, damages, losses, liabilities, and expenses arising out of your use of the services.</p>

            <h2 className="text-xl font-semibold mt-6">8. Governing Law</h2>
            <p className="mt-2">These Terms shall be governed by and construed in accordance with the laws of the State of Delaware, without regard to its conflict of law principles.</p>

            <h2 className="text-xl font-semibold mt-6">9. Changes to Terms</h2>
            <p className="mt-2">OpenAgents reserves the right to modify these Terms at any time. Your continued use of the services after any such changes constitutes your acceptance of the new Terms.</p>

            <h2 className="text-xl font-semibold mt-6">10. Contact Information</h2>
            <p className="mt-2">For any questions about these Terms, please contact us at <a href="https://twitter.com/OpenAgentsInc" target="_blank" className="font-bold">@OpenAgentsInc on X</a>.</p>
          </div>
        </CardContent>
      </Card>
    </div>
  )
}

Terms.layout = (page) => <InspectLayout children={page} title="Login" />

export default Terms
