/**
 * API endpoint for optimizing project schedule
 */

export default async (req: Request) => {
  if (req.method !== "POST") {
    return new Response("Method not allowed", { status: 405 })
  }

  try {
    await req.json()

    const scheduleId = `schedule_${Date.now()}`
    const startTime = Date.now()
    const taskTimeline: Array<any> = []

    // Mock schedule optimization - in reality this would use the actual task data
    const mockTasks = [
      { taskId: "task_0", agentId: "backend_specialist_001", duration: 4 },
      { taskId: "task_1", agentId: "backend_specialist_001", duration: 40 },
      { taskId: "task_2", agentId: "frontend_expert_001", duration: 35 },
      { taskId: "task_3", agentId: "ai_specialist_001", duration: 20 },
      { taskId: "task_4", agentId: "qa_engineer_001", duration: 15 },
      { taskId: "task_5", agentId: "doc_writer_001", duration: 10 },
      { taskId: "task_6", agentId: "qa_engineer_001", duration: 6 },
      { taskId: "task_7", agentId: "doc_writer_001", duration: 3 }
    ]

    // Simple scheduling algorithm
    const agentEndTimes = new Map<string, number>()
    const currentTime = startTime

    for (const task of mockTasks) {
      const agentAvailable = agentEndTimes.get(task.agentId) || startTime
      const taskStart = Math.max(currentTime, agentAvailable)
      const taskEnd = taskStart + (task.duration * 60 * 60 * 1000)

      taskTimeline.push({
        taskId: task.taskId,
        agentId: task.agentId,
        startTime: taskStart,
        endTime: taskEnd
      })

      agentEndTimes.set(task.agentId, taskEnd)
    }

    // Calculate total schedule duration
    const endTime = Math.max(...taskTimeline.map((t) => t.endTime))

    // Add 20% buffer for unexpected delays
    const bufferTime = (endTime - startTime) * 0.2

    const schedule = {
      scheduleId,
      startTime,
      endTime: endTime + bufferTime,
      taskTimeline,
      bufferTime,
      riskScore: 0.3 // 30% risk
    }

    return new Response(
      JSON.stringify({
        success: true,
        schedule
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
