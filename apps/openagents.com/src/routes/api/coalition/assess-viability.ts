/**
 * API endpoint for assessing coalition viability
 */

export default async (req: Request) => {
  if (req.method !== "POST") {
    return new Response("Method not allowed", { status: 405 })
  }

  try {
    const body = await req.json()
    const { agents, project } = body

    // Calculate viability based on agent skills and project requirements
    const allSkills = new Set(agents.flatMap((a: any) => a.capabilities))
    const requiredSkills = project.requirements.filter((r: any) => r.priority === "required").map((r: any) => r.skill)
    const coveredRequired = requiredSkills.filter((s: string) => allSkills.has(s))
    const skillCoverage = requiredSkills.length > 0 ? coveredRequired.length / requiredSkills.length : 1

    // Calculate average trust
    const trustLevel = agents.reduce((sum: number, a: any) => sum + a.trustScore, 0) / agents.length

    // Success probability
    let successProbability = skillCoverage * trustLevel
    const riskFactors = []
    const recommendations = []

    if (agents.length < project.minAgentsRequired) {
      riskFactors.push("Insufficient number of agents")
      successProbability *= 0.5
    }

    if (skillCoverage < 1) {
      const missingSkills = requiredSkills.filter((s: string) => !allSkills.has(s))
      recommendations.push(`Find agents with skills: ${missingSkills.join(", ")}`)
    }

    if (trustLevel < 0.8) {
      recommendations.push("Consider agents with higher trust scores")
    }

    const viability = {
      score: successProbability,
      skillCoverage,
      trustLevel,
      estimatedSuccessProbability: successProbability,
      riskFactors,
      recommendations
    }

    return new Response(
      JSON.stringify({
        success: true,
        viability
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
