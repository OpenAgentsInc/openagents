import { Button } from '@/Components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/Components/ui/card';

// Dummy data types
interface RunStats {
  revenue: number;
  apiCalls: number;
  usage: number;
  status: string;
}

interface Step {
  name: string;
  status: string;
}

interface Task {
  description: string;
  steps: Step[];
}

export const RunDetails = ({ runStats, task }: { runStats: RunStats; task: Task }) => {
  return (
    <div className="bg-black text-white p-8 rounded-lg">
      <div className="flex justify-between items-center mb-6">
        {Object.entries(runStats).map(([key, value], index) => (
          <div key={index} className="rounded-lg p-4 text-center">
            <h3 className="font-bold text-lg">{key}</h3>
            <p>{value}</p>
          </div>
        ))}
      </div>
      <Card>
        <CardHeader>
          <CardTitle>Task</CardTitle>
        </CardHeader>
        <CardContent>
          <p>{task.description}</p>
        </CardContent>
      </Card>
      <div className="my-6">
        <h2 className="text-2xl mb-4">Steps</h2>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
          {task.steps.map((step, index) => (
            <div key={index} className="rounded-lg p-4 bg-gray-800">
              <div className="flex justify-between items-center">
                <span>{index + 1}. {step.name}</span>
                <span className={`text-${step.status === 'Succeeded' ? 'green' : 'red'}-400`}>{step.status}</span>
              </div>
              <Button className="mt-4">Inspect</Button>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
};
