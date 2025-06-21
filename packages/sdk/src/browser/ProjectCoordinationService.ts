/**
 * Project Coordination Service - Multi-agent project execution and task management
 * Handles project decomposition, task assignment, progress monitoring, and deliverable integration
 */

import { Context, Data, Duration, Effect, Layer, Schedule, Schema, Stream } from "effect"
import type { Coalition, ComplexProject } from "./CoalitionFormationService.js"

// --- Task & Project Types ---
export const ProjectTask = Schema.Struct({
  taskId: Schema.String,
  projectId: Schema.String,
  title: Schema.String,
  description: Schema.String,
  requiredSkills: Schema.Array(Schema.String),
  assignedTo: Schema.optional(Schema.String), // agentId
  status: Schema.Union(
    Schema.Literal("pending"),
    Schema.Literal("assigned"),
    Schema.Literal("in_progress"),
    Schema.Literal("review"),
    Schema.Literal("completed"),
    Schema.Literal("blocked")
  ),
  dependencies: Schema.Array(Schema.String), // taskIds that must complete first
  estimatedHours: Schema.Number,
  actualHours: Schema.optional(Schema.Number),
  output: Schema.optional(Schema.String), // Task deliverable
  createdAt: Schema.Number,
  startedAt: Schema.optional(Schema.Number),
  completedAt: Schema.optional(Schema.Number)
})
export type ProjectTask = Schema.Schema.Type<typeof ProjectTask>

export const ProjectPlan = Schema.Struct({
  planId: Schema.String,
  projectId: Schema.String,
  tasks: Schema.Array(ProjectTask),
  criticalPath: Schema.Array(Schema.String), // taskIds in order
  estimatedDuration: Schema.Number, // total hours
  parallelizationFactor: Schema.Number, // how much can be done in parallel
  milestones: Schema.Array(Schema.Struct({
    name: Schema.String,
    taskIds: Schema.Array(Schema.String),
    targetDate: Schema.Number
  }))
})
export type ProjectPlan = Schema.Schema.Type<typeof ProjectPlan>

export const TaskAssignments = Schema.Struct({
  assignments: Schema.Record({ key: Schema.String, value: Schema.String }), // taskId -> agentId
  workload: Schema.Record({ key: Schema.String, value: Schema.Number }), // agentId -> total hours
  conflicts: Schema.Array(Schema.String), // taskIds that couldn't be assigned
  recommendations: Schema.Array(Schema.String)
})
export type TaskAssignments = Schema.Schema.Type<typeof TaskAssignments>

export const ProjectSchedule = Schema.Struct({
  scheduleId: Schema.String,
  startTime: Schema.Number,
  endTime: Schema.Number,
  taskTimeline: Schema.Array(Schema.Struct({
    taskId: Schema.String,
    agentId: Schema.String,
    startTime: Schema.Number,
    endTime: Schema.Number
  })),
  bufferTime: Schema.Number, // slack time built in
  riskScore: Schema.Number // 0.0 to 1.0
})
export type ProjectSchedule = Schema.Schema.Type<typeof ProjectSchedule>

// --- Progress & Quality Types ---
export const ProjectBlocker = Schema.Struct({
  blockerId: Schema.String,
  taskId: Schema.String,
  type: Schema.Union(
    Schema.Literal("dependency"),
    Schema.Literal("resource"),
    Schema.Literal("technical"),
    Schema.Literal("communication")
  ),
  description: Schema.String,
  severity: Schema.Union(Schema.Literal("low"), Schema.Literal("medium"), Schema.Literal("high")),
  blockedSince: Schema.Number,
  estimatedResolution: Schema.optional(Schema.Number)
})
export type ProjectBlocker = Schema.Schema.Type<typeof ProjectBlocker>

export const ProgressUpdate = Schema.Struct({
  timestamp: Schema.Number,
  coalitionId: Schema.String,
  projectProgress: Schema.Number, // 0.0 to 1.0
  tasksCompleted: Schema.Number,
  tasksTotal: Schema.Number,
  activeAgents: Schema.Array(Schema.String),
  blockers: Schema.Array(ProjectBlocker),
  estimatedCompletion: Schema.Number
})
export type ProgressUpdate = Schema.Schema.Type<typeof ProgressUpdate>

export const HandoffResult = Schema.Struct({
  fromTaskId: Schema.String,
  toTaskId: Schema.String,
  fromAgentId: Schema.String,
  toAgentId: Schema.String,
  deliverables: Schema.Array(Schema.String),
  handoffNotes: Schema.String,
  accepted: Schema.Boolean,
  timestamp: Schema.Number
})
export type HandoffResult = Schema.Schema.Type<typeof HandoffResult>

export const QualityReview = Schema.Struct({
  reviewId: Schema.String,
  taskId: Schema.String,
  reviewerId: Schema.String,
  score: Schema.Number, // 0.0 to 1.0
  feedback: Schema.String,
  issues: Schema.Array(Schema.String),
  approved: Schema.Boolean,
  timestamp: Schema.Number
})
export type QualityReview = Schema.Schema.Type<typeof QualityReview>

export const IntegratedDeliverable = Schema.Struct({
  deliverableId: Schema.String,
  projectId: Schema.String,
  components: Schema.Record({ key: Schema.String, value: Schema.String }), // taskId -> output
  integrationNotes: Schema.String,
  finalOutput: Schema.String,
  qualityScore: Schema.Number,
  completedAt: Schema.Number
})
export type IntegratedDeliverable = Schema.Schema.Type<typeof IntegratedDeliverable>

// --- Errors ---
export class ProjectCoordinationError extends Data.TaggedError("ProjectCoordinationError")<{
  reason:
    | "decomposition_failed"
    | "assignment_failed"
    | "scheduling_failed"
    | "monitoring_failed"
    | "handoff_failed"
    | "review_failed"
    | "integration_failed"
  message: string
  cause?: unknown
}> {}

// --- Project Coordination Service ---
export class ProjectCoordinationService extends Context.Tag("sdk/ProjectCoordinationService")<
  ProjectCoordinationService,
  {
    /**
     * Decompose complex project into manageable tasks
     */
    readonly decomposeProject: (
      project: ComplexProject
    ) => Effect.Effect<ProjectPlan, ProjectCoordinationError>

    /**
     * Assign tasks to coalition members based on skills
     */
    readonly assignTasks: (
      plan: ProjectPlan,
      coalition: Coalition
    ) => Effect.Effect<TaskAssignments, ProjectCoordinationError>

    /**
     * Optimize task schedule for efficiency
     */
    readonly optimizeSchedule: (
      assignments: TaskAssignments
    ) => Effect.Effect<ProjectSchedule, ProjectCoordinationError>

    /**
     * Monitor coalition progress in real-time
     */
    readonly monitorProgress: (
      coalition: Coalition
    ) => Stream.Stream<Array<ProgressUpdate>, ProjectCoordinationError>

    /**
     * Coordinate task handoffs between agents
     */
    readonly coordinateHandoffs: (
      completedTask: ProjectTask,
      nextTask: ProjectTask
    ) => Effect.Effect<HandoffResult, ProjectCoordinationError>

    /**
     * Manage project blockers
     */
    readonly manageBlockers: (
      blocker: ProjectBlocker
    ) => Effect.Effect<ProjectBlocker, ProjectCoordinationError>

    /**
     * Review task outputs for quality
     */
    readonly reviewTaskOutputs: (
      task: ProjectTask
    ) => Effect.Effect<QualityReview, ProjectCoordinationError>

    /**
     * Integrate task results into final deliverable
     */
    readonly integrateDeliverables: (
      taskResults: Array<ProjectTask>
    ) => Effect.Effect<IntegratedDeliverable, ProjectCoordinationError>

    /**
     * Validate project completion
     */
    readonly validateProjectCompletion: (
      deliverable: IntegratedDeliverable
    ) => Effect.Effect<{ isComplete: boolean; issues: Array<string> }, ProjectCoordinationError>
  }
>() {}

// --- Service Implementation ---
export const ProjectCoordinationServiceLive = Layer.effect(
  ProjectCoordinationService,
  Effect.sync(() => {
    // In-memory storage for active projects
    const activeProjects = new Map<string, ProjectPlan>()
    const taskProgress = new Map<string, ProjectTask>()
    const projectSchedules = new Map<string, ProjectSchedule>()

    const decomposeProject = (
      project: ComplexProject
    ): Effect.Effect<ProjectPlan, ProjectCoordinationError> =>
      Effect.try({
        try: () => {
          const planId = `plan_${project.id}_${Date.now()}`
          const tasks: Array<ProjectTask> = []

          // Create tasks based on requirements
          let taskIndex = 0
          const skillToTaskMap = new Map<string, Array<string>>()

          // Phase 1: Analysis and Planning
          if (project.requirements.some((r) => r.skill.includes("analysis") || r.skill.includes("review"))) {
            const analysisTask: ProjectTask = {
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
            skillToTaskMap.set("analysis", [analysisTask.taskId])
          }

          // Phase 2: Core Implementation Tasks
          for (const req of project.requirements) {
            const coreTask: ProjectTask = {
              taskId: `task_${taskIndex++}`,
              projectId: project.id,
              title: `Implement ${req.skill}`,
              description: `Complete ${req.skill} requirement for the project`,
              requiredSkills: [req.skill],
              status: "pending",
              dependencies: skillToTaskMap.get("analysis") || [],
              estimatedHours: req.estimatedDurationHours,
              createdAt: Date.now()
            }
            tasks.push(coreTask)

            const existing = skillToTaskMap.get(req.skill) || []
            skillToTaskMap.set(req.skill, [...existing, coreTask.taskId])
          }

          // Phase 3: Integration and Testing
          if (tasks.length > 1) {
            const integrationTask: ProjectTask = {
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
          }

          // Phase 4: Documentation and Delivery
          const deliveryTask: ProjectTask = {
            taskId: `task_${taskIndex++}`,
            projectId: project.id,
            title: "Documentation and Final Delivery",
            description: "Create documentation and prepare final deliverables",
            requiredSkills: ["documentation", "delivery"],
            status: "pending",
            dependencies: tasks.map((t) => t.taskId).slice(-2), // Depends on last few tasks
            estimatedHours: 3,
            createdAt: Date.now()
          }
          tasks.push(deliveryTask)

          // Calculate critical path (simplified - longest dependency chain)
          const criticalPath: Array<string> = []
          const findCriticalPath = (taskId: string) => {
            criticalPath.push(taskId)
            const dependents = tasks.filter((t) => t.dependencies.includes(taskId))
            if (dependents.length > 0) {
              // Choose the one with most estimated hours
              const critical = dependents.sort((a, b) => b.estimatedHours - a.estimatedHours)[0]
              findCriticalPath(critical.taskId)
            }
          }

          const rootTasks = tasks.filter((t) => t.dependencies.length === 0)
          if (rootTasks.length > 0) {
            findCriticalPath(rootTasks[0].taskId)
          }

          // Calculate total duration and parallelization
          const totalHours = tasks.reduce((sum, t) => sum + t.estimatedHours, 0)
          const maxParallel = Math.min(
            project.maxAgentsAllowed,
            tasks.filter((t) => t.dependencies.length === 0).length
          )
          const parallelizationFactor = Math.min(maxParallel, 3)

          // Create milestones
          const milestones = [
            {
              name: "Project Kickoff",
              taskIds: tasks.filter((t) => t.dependencies.length === 0).map((t) => t.taskId),
              targetDate: Date.now() + (24 * 60 * 60 * 1000)
            },
            {
              name: "Core Implementation Complete",
              taskIds: tasks.filter((t) =>
                t.requiredSkills.some((s) => project.requirements.find((r) => r.skill === s))
              ).map((t) => t.taskId),
              targetDate: Date.now() + (3 * 24 * 60 * 60 * 1000)
            },
            {
              name: "Project Completion",
              taskIds: [deliveryTask.taskId],
              targetDate: project.deadlineTimestamp
            }
          ]

          const plan: ProjectPlan = {
            planId,
            projectId: project.id,
            tasks,
            criticalPath,
            estimatedDuration: totalHours,
            parallelizationFactor,
            milestones
          }

          activeProjects.set(planId, plan)
          tasks.forEach((task) => taskProgress.set(task.taskId, task))

          return plan
        },
        catch: (error) =>
          new ProjectCoordinationError({
            reason: "decomposition_failed",
            message: `Failed to decompose project: ${error}`,
            cause: error
          })
      })

    const assignTasks = (
      plan: ProjectPlan,
      coalition: Coalition
    ): Effect.Effect<TaskAssignments, ProjectCoordinationError> =>
      Effect.try({
        try: () => {
          const assignments: Record<string, string> = {}
          const workload: Record<string, number> = {}
          const conflicts: Array<string> = []
          const recommendations: Array<string> = []

          // Initialize workload tracking
          coalition.members.forEach((member) => {
            workload[member.agentId] = 0
          })

          // Sort tasks by dependencies (tasks with no deps first)
          const sortedTasks = [...plan.tasks].sort((a, b) => a.dependencies.length - b.dependencies.length)

          // Assign tasks based on skill match and workload balance
          for (const task of sortedTasks) {
            // let assigned = false

            // Find agents with required skills
            const eligibleAgents = coalition.members.filter((member) =>
              task.requiredSkills.every((skill) => member.capabilities.includes(skill))
            )

            if (eligibleAgents.length === 0) {
              conflicts.push(task.taskId)
              recommendations.push(`No agent has skills for task ${task.title}: ${task.requiredSkills.join(", ")}`)
              continue
            }

            // Assign to agent with lowest workload
            const selectedAgent = eligibleAgents.sort((a, b) =>
              (workload[a.agentId] || 0) - (workload[b.agentId] || 0)
            )[0]

            assignments[task.taskId] = selectedAgent.agentId
            workload[selectedAgent.agentId] = (workload[selectedAgent.agentId] || 0) + task.estimatedHours
            // assigned = true

            // Update task status
            const updatedTask = { ...task, assignedTo: selectedAgent.agentId, status: "assigned" as const }
            taskProgress.set(task.taskId, updatedTask)
          }

          // Check workload balance
          const avgWorkload = Object.values(workload).reduce((sum, w) => sum + w, 0) / coalition.members.length
          const maxWorkload = Math.max(...Object.values(workload))

          if (maxWorkload > avgWorkload * 1.5) {
            recommendations.push("Workload imbalance detected - consider redistributing tasks")
          }

          if (conflicts.length > 0) {
            recommendations.push(`${conflicts.length} tasks could not be assigned due to skill gaps`)
          }

          return {
            assignments,
            workload,
            conflicts,
            recommendations
          }
        },
        catch: (error) =>
          new ProjectCoordinationError({
            reason: "assignment_failed",
            message: `Failed to assign tasks: ${error}`,
            cause: error
          })
      })

    const optimizeSchedule = (
      assignments: TaskAssignments
    ): Effect.Effect<ProjectSchedule, ProjectCoordinationError> =>
      Effect.try({
        try: () => {
          const scheduleId = `schedule_${Date.now()}`
          const startTime = Date.now()
          const taskTimeline: Array<{
            taskId: string
            agentId: string
            startTime: number
            endTime: number
          }> = []

          // Get all assigned tasks
          const assignedTasks = Array.from(taskProgress.values()).filter(
            (task) => assignments.assignments[task.taskId]
          )

          // Simple scheduling algorithm - respects dependencies
          const scheduled = new Set<string>()
          const agentEndTimes = new Map<string, number>()

          // Initialize agent availability
          Object.keys(assignments.workload).forEach((agentId) => {
            agentEndTimes.set(agentId, startTime)
          })

          // Schedule tasks
          while (scheduled.size < assignedTasks.length) {
            for (const task of assignedTasks) {
              if (scheduled.has(task.taskId)) continue

              // Check if dependencies are scheduled
              const depsComplete = task.dependencies.every((dep) => scheduled.has(dep))
              if (!depsComplete) continue

              const agentId = assignments.assignments[task.taskId]
              if (!agentId) continue

              // Find earliest start time (after dependencies and agent availability)
              let earliestStart = agentEndTimes.get(agentId) || startTime

              // Check dependency end times
              for (const depId of task.dependencies) {
                const depSchedule = taskTimeline.find((t) => t.taskId === depId)
                if (depSchedule) {
                  earliestStart = Math.max(earliestStart, depSchedule.endTime)
                }
              }

              const taskStart = earliestStart
              const taskEnd = taskStart + (task.estimatedHours * 60 * 60 * 1000)

              taskTimeline.push({
                taskId: task.taskId,
                agentId,
                startTime: taskStart,
                endTime: taskEnd
              })

              scheduled.add(task.taskId)
              agentEndTimes.set(agentId, taskEnd)
            }
          }

          // Calculate total schedule duration
          const endTime = Math.max(...taskTimeline.map((t) => t.endTime))

          // Add 20% buffer for unexpected delays
          const bufferTime = (endTime - startTime) * 0.2

          // Calculate risk score based on critical path and dependencies
          const criticalTasks = taskTimeline.filter((t) =>
            assignedTasks.find((task) => task.taskId === t.taskId)?.dependencies.length || 0 > 2
          )
          const riskScore = Math.min(1, criticalTasks.length / Math.max(1, taskTimeline.length))

          const schedule: ProjectSchedule = {
            scheduleId,
            startTime,
            endTime: endTime + bufferTime,
            taskTimeline,
            bufferTime,
            riskScore
          }

          projectSchedules.set(scheduleId, schedule)

          return schedule
        },
        catch: (error) =>
          new ProjectCoordinationError({
            reason: "scheduling_failed",
            message: `Failed to optimize schedule: ${error}`,
            cause: error
          })
      })

    const monitorProgress = (
      coalition: Coalition
    ): Stream.Stream<Array<ProgressUpdate>, ProjectCoordinationError> =>
      Stream.repeat(
        Effect.try({
          try: () => {
            const coalitionTasks = Array.from(taskProgress.values()).filter(
              (task) => task.projectId === coalition.contract.proposal.project.id
            )

            const completed = coalitionTasks.filter((t) => t.status === "completed").length
            const total = coalitionTasks.length
            const projectProgress = total > 0 ? completed / total : 0

            // Identify blockers
            const blockers: Array<ProjectBlocker> = coalitionTasks
              .filter((t) => t.status === "blocked")
              .map((task) => ({
                blockerId: `blocker_${task.taskId}`,
                taskId: task.taskId,
                type: "dependency" as const,
                description: `Task ${task.title} is blocked`,
                severity: "medium" as const,
                blockedSince: Date.now()
              }))

            // Estimate completion based on current progress
            // const inProgress = coalitionTasks.filter(t => t.status === "in_progress").length
            const remainingHours = coalitionTasks
              .filter((t) => t.status !== "completed")
              .reduce((sum, t) => sum + t.estimatedHours, 0)

            const avgHoursPerDay = 8 * coalition.members.length
            const daysRemaining = remainingHours / avgHoursPerDay
            const estimatedCompletion = Date.now() + (daysRemaining * 24 * 60 * 60 * 1000)

            const update: ProgressUpdate = {
              timestamp: Date.now(),
              coalitionId: coalition.coalitionId,
              projectProgress,
              tasksCompleted: completed,
              tasksTotal: total,
              activeAgents: coalition.members.filter((m) => m.isAvailable).map((m) => m.agentId),
              blockers,
              estimatedCompletion
            }

            return [update]
          },
          catch: (error) =>
            new ProjectCoordinationError({
              reason: "monitoring_failed",
              message: `Failed to monitor progress: ${error}`,
              cause: error
            })
        }),
        Schedule.spaced(Duration.seconds(30))
      )

    const coordinateHandoffs = (
      completedTask: ProjectTask,
      nextTask: ProjectTask
    ): Effect.Effect<HandoffResult, ProjectCoordinationError> =>
      Effect.try({
        try: () => {
          // Validate handoff is possible
          if (!completedTask.assignedTo || !nextTask.assignedTo) {
            throw new Error("Tasks must be assigned for handoff")
          }

          if (completedTask.status !== "completed") {
            throw new Error("Source task must be completed for handoff")
          }

          const handoff: HandoffResult = {
            fromTaskId: completedTask.taskId,
            toTaskId: nextTask.taskId,
            fromAgentId: completedTask.assignedTo,
            toAgentId: nextTask.assignedTo,
            deliverables: [completedTask.output || ""],
            handoffNotes: `Handoff from ${completedTask.title} to ${nextTask.title}`,
            accepted: true,
            timestamp: Date.now()
          }

          // Update next task status
          const updatedTask = {
            ...nextTask,
            status: "in_progress" as const,
            startedAt: Date.now()
          }
          taskProgress.set(nextTask.taskId, updatedTask)

          return handoff
        },
        catch: (error) =>
          new ProjectCoordinationError({
            reason: "handoff_failed",
            message: `Failed to coordinate handoff: ${error}`,
            cause: error
          })
      })

    const manageBlockers = (
      blocker: ProjectBlocker
    ): Effect.Effect<ProjectBlocker, ProjectCoordinationError> =>
      Effect.try({
        try: () => {
          // Simple blocker resolution logic
          const task = taskProgress.get(blocker.taskId)
          if (!task) {
            throw new Error(`Task ${blocker.taskId} not found`)
          }

          // Update estimated resolution based on blocker type
          let estimatedResolution = Date.now()

          switch (blocker.type) {
            case "dependency": {
              // Check if dependencies are complete
              const depsComplete = task.dependencies.every((depId) => {
                const dep = taskProgress.get(depId)
                return dep && dep.status === "completed"
              })

              if (depsComplete) {
                // Unblock the task
                const unblocked = { ...task, status: "assigned" as const }
                taskProgress.set(task.taskId, unblocked)
                estimatedResolution = Date.now()
              } else {
                estimatedResolution = Date.now() + (4 * 60 * 60 * 1000) // 4 hours
              }
              break
            }

            case "resource":
              estimatedResolution = Date.now() + (2 * 60 * 60 * 1000) // 2 hours
              break

            case "technical":
              estimatedResolution = Date.now() + (8 * 60 * 60 * 1000) // 8 hours
              break

            case "communication":
              estimatedResolution = Date.now() + (1 * 60 * 60 * 1000) // 1 hour
              break
          }

          return {
            ...blocker,
            estimatedResolution
          }
        },
        catch: (error) =>
          new ProjectCoordinationError({
            reason: "handoff_failed",
            message: `Failed to manage blocker: ${error}`,
            cause: error
          })
      })

    const reviewTaskOutputs = (
      task: ProjectTask
    ): Effect.Effect<QualityReview, ProjectCoordinationError> =>
      Effect.try({
        try: () => {
          if (!task.output) {
            throw new Error("Task has no output to review")
          }

          // Simple quality scoring based on task completion
          let score = 0.8 // Base score
          const issues: Array<string> = []

          // Check if task was completed on time
          if (task.completedAt && task.startedAt) {
            const actualDuration = (task.completedAt - task.startedAt) / (60 * 60 * 1000)
            if (actualDuration > task.estimatedHours * 1.2) {
              score -= 0.1
              issues.push("Task took longer than estimated")
            }
          }

          // Check output quality (simplified)
          if (task.output.length < 100) {
            score -= 0.1
            issues.push("Output seems too brief")
          }

          const review: QualityReview = {
            reviewId: `review_${task.taskId}_${Date.now()}`,
            taskId: task.taskId,
            reviewerId: "system", // In production, another agent would review
            score,
            feedback: issues.length > 0 ? `Issues found: ${issues.join(", ")}` : "Task completed satisfactorily",
            issues,
            approved: score >= 0.6,
            timestamp: Date.now()
          }

          return review
        },
        catch: (error) =>
          new ProjectCoordinationError({
            reason: "review_failed",
            message: `Failed to review task outputs: ${error}`,
            cause: error
          })
      })

    const integrateDeliverables = (
      taskResults: Array<ProjectTask>
    ): Effect.Effect<IntegratedDeliverable, ProjectCoordinationError> =>
      Effect.try({
        try: () => {
          if (taskResults.length === 0) {
            throw new Error("No task results to integrate")
          }

          const projectId = taskResults[0].projectId
          const components: Record<string, string> = {}

          // Collect all outputs
          taskResults.forEach((task) => {
            if (task.output) {
              components[task.taskId] = task.output
            }
          })

          // Create integrated output (simplified - concatenate all outputs)
          const finalOutput = Object.entries(components)
            .map(([taskId, output]) => {
              const task = taskResults.find((t) => t.taskId === taskId)
              return `## ${task?.title || taskId}\n\n${output}\n`
            })
            .join("\n---\n\n")

          // Calculate overall quality score
          const qualityScore = taskResults.reduce((sum, task) => {
            return sum + (task.status === "completed" ? 0.9 : 0.5)
          }, 0) / taskResults.length

          const deliverable: IntegratedDeliverable = {
            deliverableId: `deliverable_${projectId}_${Date.now()}`,
            projectId,
            components,
            integrationNotes: `Integrated ${taskResults.length} task outputs`,
            finalOutput,
            qualityScore,
            completedAt: Date.now()
          }

          return deliverable
        },
        catch: (error) =>
          new ProjectCoordinationError({
            reason: "integration_failed",
            message: `Failed to integrate deliverables: ${error}`,
            cause: error
          })
      })

    const validateProjectCompletion = (
      deliverable: IntegratedDeliverable
    ): Effect.Effect<{ isComplete: boolean; issues: Array<string> }, ProjectCoordinationError> =>
      Effect.try({
        try: () => {
          const issues: Array<string> = []

          // Check quality score
          if (deliverable.qualityScore < 0.7) {
            issues.push("Overall quality score is below threshold")
          }

          // Check if all components are present
          const plan = Array.from(activeProjects.values()).find(
            (p) => p.projectId === deliverable.projectId
          )

          if (plan) {
            const missingTasks = plan.tasks.filter(
              (task) => !deliverable.components[task.taskId]
            )

            if (missingTasks.length > 0) {
              issues.push(`Missing outputs from ${missingTasks.length} tasks`)
            }
          }

          // Check final output
          if (deliverable.finalOutput.length < 500) {
            issues.push("Final deliverable seems incomplete")
          }

          return {
            isComplete: issues.length === 0,
            issues
          }
        },
        catch: (error) =>
          new ProjectCoordinationError({
            reason: "integration_failed",
            message: `Failed to validate project completion: ${error}`,
            cause: error
          })
      })

    return {
      decomposeProject,
      assignTasks,
      optimizeSchedule,
      monitorProgress,
      coordinateHandoffs,
      manageBlockers,
      reviewTaskOutputs,
      integrateDeliverables,
      validateProjectCompletion
    }
  })
)
