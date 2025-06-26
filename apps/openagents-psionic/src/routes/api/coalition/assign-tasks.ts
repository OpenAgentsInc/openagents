/**
 * API endpoint for assigning tasks to coalition members
 */

export default async (req: Request) => {
  if (req.method !== "POST") {
    return new Response("Method not allowed", { status: 405 })
  }

  try {
    const body = await req.json()
    const { coalition, plan } = body

    const assignments: Record<string, string> = {}
    const workload: Record<string, number> = {}
    const conflicts: Array<string> = []
    const recommendations: Array<string> = []

    // Initialize workload tracking
    coalition.members.forEach((member: any) => {
      workload[member.agentId] = 0
    })

    // Sort tasks by dependencies (tasks with no deps first)
    const sortedTasks = [...plan.tasks].sort((a: any, b: any) => a.dependencies.length - b.dependencies.length)

    // Assign tasks based on skill match and workload balance
    for (const task of sortedTasks) {
      // const assigned = false

      // Find agents with required skills
      const eligibleAgents = coalition.members.filter((member: any) =>
        task.requiredSkills.every((skill: string) => member.capabilities.includes(skill))
      )

      if (eligibleAgents.length === 0) {
        conflicts.push(task.taskId)
        recommendations.push(`No agent has skills for task ${task.title}: ${task.requiredSkills.join(", ")}`)
        continue
      }

      // Assign to agent with lowest workload
      const selectedAgent = eligibleAgents.sort((a: any, b: any) =>
        (workload[a.agentId] || 0) - (workload[b.agentId] || 0)
      )[0]

      assignments[task.taskId] = selectedAgent.agentId
      workload[selectedAgent.agentId] = (workload[selectedAgent.agentId] || 0) + task.estimatedHours
      // assigned = true
    }

    // Check workload balance
    const avgWorkload = Object.values(workload).reduce((sum: any, w: any) => sum + w, 0) / coalition.members.length
    const maxWorkload = Math.max(...Object.values(workload) as Array<number>)

    if (maxWorkload > avgWorkload * 1.5) {
      recommendations.push("Workload imbalance detected - consider redistributing tasks")
    }

    if (conflicts.length > 0) {
      recommendations.push(`${conflicts.length} tasks could not be assigned due to skill gaps`)
    }

    return new Response(
      JSON.stringify({
        success: true,
        assignments: {
          assignments,
          workload,
          conflicts,
          recommendations
        }
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
