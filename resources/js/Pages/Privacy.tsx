import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/Components/ui/card'
import { NavLayout } from '@/Layouts/NavLayout'

function Privacy() {
  return (
    <div className="min-h-screen w-screen flex flex-col justify-center items-center -pt-12 px-4 w-auto">
      <Card className="px-4 my-12 w-full sm:w-2/3">
        <CardHeader>
          <CardTitle>Privacy Policy</CardTitle>
          <CardDescription>Last updated December 22, 2023</CardDescription>
        </CardHeader>
        <CardContent className="text-muted-foreground space-y-4 -mt-2 text-sm">
          <div className="container mx-auto px-4">
            <h2 className="text-xl font-semibold mt-6">1. Introduction</h2>
            <p className="mt-2">This Privacy Policy describes how OpenAgents, Inc. ("OpenAgents") collects, uses, and discloses information, and what choices you have with respect to the information.</p>

            <h2 className="text-xl font-semibold mt-6">2. Information Collection and Use</h2>
            <p className="mt-2">We collect various types of information, including information you provide directly, information collected automatically through your use of our services, and information obtained from third-party sources.</p>
            <p className="mt-2"><strong>2.1 Information You Provide:</strong> This may include personal information such as your name, email address, user content, etc.</p>
            <p className="mt-2"><strong>2.2 Automatically Collected Information:</strong> We collect information about your interaction with our services, like IP address, device information, and viewed pages.</p>
            <p className="mt-2"><strong>2.3 Third-Party Sources:</strong> We may receive information about you from other sources, including publicly available databases or third parties, and combine this data with information we already have about you.</p>

            <h2 className="text-xl font-semibold mt-6">3. Use of Information</h2>
            <p className="mt-2">The information we collect is used to provide, maintain, and improve our services, to develop new services, and to protect OpenAgents and our users.</p>

            <h2 className="text-xl font-semibold mt-6">4. Information Sharing and Disclosure</h2>
            <p className="mt-2">We may share your information with third-party service providers, to comply with legal obligations, to protect and defend our rights and property, or in the case of an acquisition, sale, or merger.</p>

            <h2 className="text-xl font-semibold mt-6">5. Data Retention</h2>
            <p className="mt-2">OpenAgents retains the personal information we receive as described in this Privacy Policy for as long as necessary to fulfill the purpose(s) for which it was collected, comply with legal obligations, resolve disputes, enforce our agreements, and other permissible purposes.</p>

            <h2 className="text-xl font-semibold mt-6">6. Your Rights and Choices</h2>
            <p className="mt-2">Depending on your location, you may have certain rights regarding the information we hold about you, including the right to access, correct, or delete your personal information.</p>

            <h2 className="text-xl font-semibold mt-6">7. Security</h2>
            <p className="mt-2">We take reasonable measures to protect your personal information from loss, theft, misuse, and unauthorized access, disclosure, alteration, and destruction.</p>

            <h2 className="text-xl font-semibold mt-6">8. International Transfers</h2>
            <p className="mt-2">Your information may be transferred to, and maintained on, computers located outside of your state, province, country, or other governmental jurisdiction where the data protection laws may differ from those of your jurisdiction.</p>

            <h2 className="text-xl font-semibold mt-6">9. Updates to this Policy</h2>
            <p className="mt-2">We may update this Privacy Policy from time to time. We will notify you of any changes by posting the new Privacy Policy on this page.</p>

            <h2 className="text-xl font-semibold mt-6">10. Contact Us</h2>
            <p className="mt-2">If you have any questions about this Privacy Policy, please contact us at <a href="https://twitter.com/OpenAgentsInc" target="_blank" className="font-bold">@OpenAgentsInc on X</a>.</p>

          </div>
        </CardContent>
      </Card>
    </div>
  )
}

Privacy.layout = (page) => <NavLayout children={page} title="Login" />

export default Privacy
