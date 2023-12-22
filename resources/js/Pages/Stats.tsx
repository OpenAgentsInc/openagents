import { usePage } from "@inertiajs/react"

export default function Stats() {
  const { userBalanceSum, userCount } = usePage().props as any
  return (
    <div>
      <h1>Stats</h1>
      <p>Users: {userCount}</p>
      <p>User balance total: {userBalanceSum}</p>
    </div>
  )
}
