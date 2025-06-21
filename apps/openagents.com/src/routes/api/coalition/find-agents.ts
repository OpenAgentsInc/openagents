/**
 * API endpoint for finding complementary agents
 */

export default async (req: Request) => {
  if (req.method !== "POST") {
    return new Response("Method not allowed", { status: 405 })
  }

  try {
    await req.json()

    // Mock agent matches for demo
    const mockAgents = [
      {
        agent: {
          agentId: "backend_specialist_001",
          publicKey: "npub_backend_001",
          personality: {
            name: "BackendPro",
            role: "developer",
            serviceSpecializations: ["backend-development", "database-design", "api-development"]
          },
          capabilities: ["backend-development", "testing", "optimization"],
          averageRating: 4.8,
          completedJobs: 156,
          trustScore: 0.92,
          currentBalance: 75000,
          isAvailable: true
        },
        matchScore: 0.95,
        matchedSkills: ["backend-development"],
        missingSkills: [],
        estimatedContribution: 0.35
      },
      {
        agent: {
          agentId: "frontend_expert_001",
          publicKey: "npub_frontend_001",
          personality: {
            name: "UIWizard",
            role: "designer-developer",
            serviceSpecializations: ["frontend-development", "ui-design", "user-experience"]
          },
          capabilities: ["frontend-development", "ui-design", "responsive-design"],
          averageRating: 4.7,
          completedJobs: 203,
          trustScore: 0.89,
          currentBalance: 65000,
          isAvailable: true
        },
        matchScore: 0.90,
        matchedSkills: ["frontend-development"],
        missingSkills: [],
        estimatedContribution: 0.30
      },
      {
        agent: {
          agentId: "ai_specialist_001",
          publicKey: "npub_ai_001",
          personality: {
            name: "AIGuru",
            role: "ai-engineer",
            serviceSpecializations: ["ai-integration", "machine-learning", "data-analysis"]
          },
          capabilities: ["ai-integration", "model-training", "optimization"],
          averageRating: 4.9,
          completedJobs: 89,
          trustScore: 0.95,
          currentBalance: 120000,
          isAvailable: true
        },
        matchScore: 0.88,
        matchedSkills: ["ai-integration"],
        missingSkills: [],
        estimatedContribution: 0.20
      },
      {
        agent: {
          agentId: "qa_engineer_001",
          publicKey: "npub_qa_001",
          personality: {
            name: "TestMaster",
            role: "qa-engineer",
            serviceSpecializations: ["testing", "quality-assurance", "automation"]
          },
          capabilities: ["testing", "test-automation", "bug-tracking"],
          averageRating: 4.6,
          completedJobs: 134,
          trustScore: 0.87,
          currentBalance: 45000,
          isAvailable: true
        },
        matchScore: 0.75,
        matchedSkills: ["testing"],
        missingSkills: [],
        estimatedContribution: 0.10
      },
      {
        agent: {
          agentId: "doc_writer_001",
          publicKey: "npub_doc_001",
          personality: {
            name: "DocSmith",
            role: "technical-writer",
            serviceSpecializations: ["documentation", "api-docs", "user-guides"]
          },
          capabilities: ["documentation", "technical-writing", "tutorial-creation"],
          averageRating: 4.5,
          completedJobs: 178,
          trustScore: 0.85,
          currentBalance: 40000,
          isAvailable: true
        },
        matchScore: 0.70,
        matchedSkills: ["documentation"],
        missingSkills: [],
        estimatedContribution: 0.05
      }
    ]

    return new Response(
      JSON.stringify({
        success: true,
        matches: mockAgents
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
