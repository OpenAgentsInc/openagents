/**
 * API endpoint for negotiating coalition terms
 */

export default async (req: Request) => {
  if (req.method !== "POST") {
    return new Response("Method not allowed", { status: 405 })
  }

  try {
    const body = await req.json()
    const { proposal } = body

    const contractId = `contract_${proposal.proposalId}`
    const signatures: Record<string, string> = {}

    // Simulate each agent reviewing and signing
    for (const agent of proposal.proposedMembers) {
      // Simulate agent review logic
      const acceptanceChance = agent.trustScore * (proposal.paymentSplits[agent.agentId] / 100)

      if (acceptanceChance > 0.3) { // Accept if reasonable
        signatures[agent.agentId] = `sig_${agent.agentId}_${Date.now()}`
      }
    }

    // Check if all agents signed
    const allSigned = proposal.proposedMembers.every((agent: any) => signatures[agent.agentId])

    if (!allSigned) {
      return new Response(
        JSON.stringify({
          success: false,
          error: "Not all agents agreed to the terms"
        }),
        {
          status: 400,
          headers: { "Content-Type": "application/json" }
        }
      )
    }

    // Create escrow address (mock)
    const escrowAddress = `bc1q_escrow_${contractId.substring(9, 20)}`

    const contract = {
      contractId,
      proposal,
      signatures,
      escrowAddress,
      status: "pending",
      createdAt: Date.now()
    }

    return new Response(
      JSON.stringify({
        success: true,
        contract
      }),
      {
        headers: { "Content-Type": "application/json" }
      }
    )
  } catch (error) {
    return new Response(
      JSON.stringify({
        success: false,
        error: error instanceof Error ? error.message : "Unknown error"
      }),
      {
        status: 500,
        headers: { "Content-Type": "application/json" }
      }
    )
  }
}
