import { Card, CardContent, CardHeader, CardTitle } from '@/Components/ui/card';
import { StepStats } from './StepStats';

export interface Step {
    id: number
    run_id: number
    name: string;
    status: string;
    description: string;
}

export interface Task {
    description: string;
    steps: Step[];
}

export const StepDetails = ({ step }: { step: Step }) => {
    return (
        <div className="pt-6 px-8 rounded-lg">
            <StepStats />
            <Card>
                <CardHeader>
                    <CardTitle>Step</CardTitle>
                </CardHeader>
                <CardContent>
                    <p>{step.description}</p>
                </CardContent>
            </Card>
            <div className="my-6">

            </div>
        </div>
    );
};
