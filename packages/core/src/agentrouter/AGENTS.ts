export const AGENTS = [
  {
    name: 'Coder',
    description: 'Makes code changes',
  },
  {
    name: 'Researcher',
    description: 'Researches information',
  },
  {
    name: 'Project Manager',
    description: 'Manages projects',
  },
  {
    name: 'Writer',
    description: 'Writes content',
  },
  {
    name: 'Indexer',
    description: 'Indexes codebases & data',
  },
] as const

export type AgentName = (typeof AGENTS)[number]['name']
