/**
 * API endpoint for proposing a coalition
 */

export default async (req: Request) => {
  if (req.method !== "POST") {
    return new Response("Method not allowed", { status: 405 })
  }

  try {
    const body = await req.json()
    const { agents, project } = body

    const proposalId = `coalition_${Date.now()}_${Math.random().toString(36).substring(7)}`

    // Calculate payment splits based on contribution
    const paymentSplits: Record<string, number> = {}
    let totalContribution = 0

    // Assess each agent's contribution
    const contributions = agents.map((agent: any) => {
      const matchingSkills = project.requirements.filter((req: any) => agent.capabilities.includes(req.skill))

      // Weight by skill priority and agent trust score
      const contribution = matchingSkills.reduce((sum: number, skill: any) => {
        const weight = skill.priority === "required" ? 1.0 : skill.priority === "preferred" ? 0.7 : 0.3
        return sum + (weight * agent.trustScore)
      }, 0)

      return { agent, contribution }
    })

    totalContribution = contributions.reduce((sum: number, c: any) => sum + c.contribution, 0)

    // Assign payment splits
    contributions.forEach(({ agent, contribution }: any) => {
      paymentSplits[agent.agentId] = Math.floor((contribution / totalContribution) * 100)
    })

    // Ensure splits add up to 100%
    const totalSplit = Object.values(paymentSplits).reduce((sum: any, split: any) => sum + split, 0)
    if (totalSplit < 100) {
      const topAgent = contributions.sort((a: any, b: any) => b.contribution - a.contribution)[0].agent
      paymentSplits[topAgent.agentId] += 100 - totalSplit
    }

    // Assign tasks based on capabilities
    const taskAssignments: Record<string, Array<string>> = {}

    agents.forEach((agent: any) => {
      taskAssignments[agent.agentId] = project.requirements
        .filter((req: any) => agent.capabilities.includes(req.skill))
        .map((req: any) => req.skill)
    })

    // Estimate completion time
    const totalHours = project.requirements.reduce((sum: number, req: any) => sum + req.estimatedDurationHours, 0)
    const parallelFactor = Math.min(agents.length, 3)
    const estimatedHours = totalHours / parallelFactor
    const estimatedCompletionTime = Date.now() + (estimatedHours * 60 * 60 * 1000)

    const proposal = {
      proposalId,
      project,
      proposedMembers: agents,
      paymentSplits,
      taskAssignments,
      estimatedCompletionTime,
      proposedBy: agents[0].agentId,
      expiresAt: Date.now() + (24 * 60 * 60 * 1000)
    }

    return new Response(
      JSON.stringify({
        success: true,
        proposal
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
