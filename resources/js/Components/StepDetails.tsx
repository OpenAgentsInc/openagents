import { Card, CardContent, CardHeader, CardTitle } from '@/Components/ui/card';
import { StepStats } from './StepStats';

export interface Step {
    id: number
    run_id: number
    name: string;
    status: string;
    description: string;
    input: any
    output: any
}

export interface Task {
    description: string;
    steps: Step[];
}

export const StepDetails = ({ step }: { step: any }) => {
    return (
        <div className="pt-6 px-8 rounded-lg">
            {/* <StepStats /> */}
            <Card>
                <CardHeader>
                    <CardTitle>Step</CardTitle>
                </CardHeader>
                <CardContent>
                    <p>{step.description}</p>
                </CardContent>
            </Card>
            <Card className="mt-6">
                <CardContent>
                    <div className="mt-6 flex flex-wrap -mx-4">
                        <div className="w-full lg:w-1/2 px-4">
                            <div className="mb-4">
                                <h5 className="text-md font-semibold mb-2">Input</h5>
                                <div className="rounded-lg border p-4 bg-gray-100 overflow-auto">
                                    <pre className="text-sm whitespace-pre-wrap">{step.input}</pre>
                                </div>
                            </div>
                        </div>
                        <div className="w-full lg:w-1/2 px-4">
                            <div className="mb-4">
                                <h5 className="text-md font-semibold mb-2">Output</h5>
                                <div className="rounded-lg border p-4 bg-gray-100 overflow-auto">
                                    <pre className="text-sm whitespace-pre-wrap">{step.output}</pre>
                                </div>
                            </div>
                        </div>
                    </div>
                </CardContent>
            </Card>
        </div>
    );
};
