export interface LabelInterface {
   id: string;
   name: string;
   color: string;
}

export const labels: LabelInterface[] = [
   { id: 'ui', name: 'UI Enhancement', color: 'purple' },
   { id: 'bug', name: 'Bug', color: 'red' },
   { id: 'feature', name: 'Feature', color: 'green' },
   { id: 'documentation', name: 'Documentation', color: 'blue' },
   { id: 'refactor', name: 'Refactor', color: 'yellow' },
   { id: 'performance', name: 'Performance', color: 'orange' },
   { id: 'design', name: 'Design', color: 'pink' },
   { id: 'security', name: 'Security', color: 'gray' },
   { id: 'accessibility', name: 'Accessibility', color: 'indigo' },
   { id: 'testing', name: 'Testing', color: 'teal' },
   { id: 'internationalization', name: 'Internationalization', color: 'cyan' },
];
