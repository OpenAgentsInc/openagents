import { Button } from '@/Components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/Components/ui/card';
import { Stats } from './RunStats';
import { router } from "@inertiajs/react"

// Dummy data types
interface RunStats {
    revenue: number;
    apiCalls: number;
    usage: number;
    status: string;
}

interface Step {
    id: number
    name: string;
    status: string;
    description: string;
}

interface Task {
    description: string;
    steps: Step[];
}

export const RunDetails = ({ runStats, task }: { runStats: RunStats; task: Task }) => {
    // console.log(task)
    return (
        <div className="pt-6 px-8 rounded-lg">
            <Stats />
            <Card>
                <CardHeader>
                    <CardTitle>Task</CardTitle>
                </CardHeader>
                <CardContent>
                    <p>{task.description}</p>
                </CardContent>
            </Card>
            <div className="my-6">
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
                    {task.steps.map((step, index) => (
                        <Card key={index}
                            onClick={() => {
                                router.get(`/step/${step.id}`)
                            }}
                            style={{ cursor: 'pointer' }}
                        >
                            <CardHeader>
                                <CardTitle>Step {index + 1}</CardTitle>
                                <CardDescription>{step.description}</CardDescription>
                            </CardHeader>
                            {/* <CardContent>
                                <div className="flex justify-between items-center">
                                    <span className={`text-${step.status === 'success' ? 'green' : 'red'}-400`}>{step.status}</span>
                                </div>
                            </CardContent> */}
                        </Card>
                    ))}
                </div>
            </div>
        </div>
    );
};
