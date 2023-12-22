import { usePage } from "@inertiajs/react"

export default function Referrals() {
  const props = usePage().props as any
  const referrals = props.referrals
  console.log(props)
  return <>

    <h1>Your referral link:</h1>
    <p>https://openagents.com/?r={props.auth.user.github_nickname}</p>

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
  </>
}
