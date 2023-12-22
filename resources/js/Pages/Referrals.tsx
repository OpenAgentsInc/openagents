import { usePage } from "@inertiajs/react"

export default function Referrals() {
  const props = usePage().props as any
  const referrals = props.referrals
  console.log(referrals)
  return <>
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
