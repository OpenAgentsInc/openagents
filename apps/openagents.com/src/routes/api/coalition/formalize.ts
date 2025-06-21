/**
 * API endpoint for formalizing coalition agreement
 */

export default async (req: Request) => {
  if (req.method !== "POST") {
    return new Response("Method not allowed", { status: 405 })
  }

  try {
    const body = await req.json()
    const { contract } = body

    const coalitionId = `coalition_${contract.contractId.substring(9)}`

    // Create internal coordination channel
    const internalChannelId = `channel_${coalitionId}`

    // Initialize coalition
    const coalition = {
      coalitionId,
      contract: {
        ...contract,
        status: "active",
        activatedAt: Date.now()
      },
      members: contract.proposal.proposedMembers,
      projectProgress: 0,
      activeTasks: [],
      completedTasks: [],
      internalChannelId
    }

    return new Response(
      JSON.stringify({
        success: true,
        coalition
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
