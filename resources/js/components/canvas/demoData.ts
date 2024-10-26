export const demoNodes = [
  { id: '1', name: 'LLM Chat', type: 'feature' },
  { id: '2', name: 'Coding Agent', type: 'feature' },
  { id: '3', name: 'GitHub Integration', type: 'feature' },
  { id: '4', name: 'Codebase Analysis', type: 'feature' },
  { id: '5', name: 'Bitcoin Payments', type: 'feature' },
];

export const demoEdges = [
  { source: '1', target: '2', type: 'controls' },
  { source: '2', target: '3', type: 'uses' },
  { source: '3', target: '4', type: 'enables' },
  { source: '4', target: '5', type: 'earns' },
  { source: '5', target: '1', type: 'pay for' },
];
