/**
 * API endpoint for decomposing project into tasks
 */

export default async (req: Request) => {
  if (req.method !== "POST") {
    return new Response("Method not allowed", { status: 405 })
  }

  try {
    const body = await req.json()
    const { project } = body

    const planId = `plan_${project.id}_${Date.now()}`
    const tasks: Array<any> = []
    let taskIndex = 0

    // Phase 1: Analysis and Planning
    const analysisTask = {
      taskId: `task_${taskIndex++}`,
      projectId: project.id,
      title: "Initial Analysis and Requirements Review",
      description: "Analyze project requirements and create detailed plan",
      requiredSkills: ["analysis", "planning"],
      status: "pending",
      dependencies: [],
      estimatedHours: 4,
      createdAt: Date.now()
    }
    tasks.push(analysisTask)

    // Phase 2: Core Implementation Tasks
    for (const req of project.requirements) {
      const coreTask = {
        taskId: `task_${taskIndex++}`,
        projectId: project.id,
        title: `Implement ${req.skill}`,
        description: `Complete ${req.skill} requirement for the project`,
        requiredSkills: [req.skill],
        status: "pending",
        dependencies: [analysisTask.taskId],
        estimatedHours: req.estimatedDurationHours,
        createdAt: Date.now()
      }
      tasks.push(coreTask)
    }

    // Phase 3: Integration and Testing
    const integrationTask = {
      taskId: `task_${taskIndex++}`,
      projectId: project.id,
      title: "Integration and Testing",
      description: "Integrate all components and perform testing",
      requiredSkills: ["testing", "integration"],
      status: "pending",
      dependencies: tasks.filter((t) => t.dependencies.length > 0).map((t) => t.taskId),
      estimatedHours: Math.max(4, tasks.length * 0.5),
      createdAt: Date.now()
    }
    tasks.push(integrationTask)

    // Phase 4: Documentation and Delivery
    const deliveryTask = {
      taskId: `task_${taskIndex++}`,
      projectId: project.id,
      title: "Documentation and Final Delivery",
      description: "Create documentation and prepare final deliverables",
      requiredSkills: ["documentation", "delivery"],
      status: "pending",
      dependencies: [integrationTask.taskId],
      estimatedHours: 3,
      createdAt: Date.now()
    }
    tasks.push(deliveryTask)

    // Calculate critical path
    const criticalPath = [analysisTask.taskId]
    const coreTasks = tasks.filter((t) => project.requirements.some((r: any) => t.requiredSkills.includes(r.skill)))
    if (coreTasks.length > 0) {
      criticalPath.push(coreTasks[0].taskId)
    }
    criticalPath.push(integrationTask.taskId)
    criticalPath.push(deliveryTask.taskId)

    // Create milestones
    const milestones = [
      {
        name: "Project Kickoff",
        taskIds: [analysisTask.taskId],
        targetDate: Date.now() + (24 * 60 * 60 * 1000)
      },
      {
        name: "Core Implementation Complete",
        taskIds: coreTasks.map((t) => t.taskId),
        targetDate: Date.now() + (3 * 24 * 60 * 60 * 1000)
      },
      {
        name: "Project Completion",
        taskIds: [deliveryTask.taskId],
        targetDate: project.deadlineTimestamp
      }
    ]

    const plan = {
      planId,
      projectId: project.id,
      tasks,
      criticalPath,
      estimatedDuration: tasks.reduce((sum, t) => sum + t.estimatedHours, 0),
      parallelizationFactor: Math.min(project.maxAgentsAllowed, 3),
      milestones
    }

    return new Response(
      JSON.stringify({
        success: true,
        plan
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
