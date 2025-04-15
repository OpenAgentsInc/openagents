export interface Cycle {
   id: string;
   number: number;
   name: string;
   teamId: string;
   startDate: string;
   endDate: string;
   progress: number;
}

export const cycles: Cycle[] = [
   {
      id: '42',
      number: 42,
      name: 'Sprint 42 - Pixel Perfect',
      teamId: 'design-system',
      startDate: '2025-03-10',
      endDate: '2025-03-24',
      progress: 80,
   },
   {
      id: '43',
      number: 43,
      name: 'Sprint 43 - Performance Boost',
      teamId: 'performance-lab',
      startDate: '2025-03-10',
      endDate: '2025-03-24',
      progress: 50,
   },
   {
      id: '44',
      number: 44,
      name: 'Sprint 44 - Core Enhancements',
      teamId: 'lndev-core',
      startDate: '2025-03-10',
      endDate: '2025-03-24',
      progress: 0,
   },
];
