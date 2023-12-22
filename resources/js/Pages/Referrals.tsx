import { usePage } from "@inertiajs/react"
import AuthenticatedLayout from '@/Layouts/AuthenticatedLayout'
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "../Components/ui/card"
import { Button } from "@/Components/ui/button"

export default function Referrals() {
  const props = usePage().props as any
  const referrals = props.referrals
  return <AuthenticatedLayout user={props.auth.user}>

    <Card className="mt-24 mx-auto w-[350px]">
      <CardHeader>
        <CardTitle>Your referral link</CardTitle>
        <CardDescription>Earn bitcoin when your referrals spend</CardDescription>
      </CardHeader>

      <CardContent>
        <p>https://openagents.com/?r={props.auth.user.github_nickname}</p>
      </CardContent>
      <CardFooter>
        <Button>Share</Button>
      </CardFooter>
    </Card>

    <h1>Referrals ({referrals.length})</h1>

    <table>
      <tbody>
        {referrals.map((referral: any) => {
          return <tr key={referral.id}>
            <td>{referral.github_nickname}</td>
          </tr>
        })}
      </tbody>
    </table>
  </AuthenticatedLayout>
}
