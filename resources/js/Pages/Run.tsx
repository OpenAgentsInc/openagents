import InspectLayout from '@/Layouts/InspectLayout';
import { usePage } from '@inertiajs/react';
import { RunDetails } from '@/Components/RunDetails';
import { Run as RunType } from '@/Components/RunTable';

const Run = () => {
    const { props } = usePage();
    const run = props.run as RunType;

    // Assuming `run` contains the stats and task data, otherwise, you would fetch or calculate them here.
    const runStats = {
        revenue: 123, // run.revenue,
        apiCalls: 12, // run.apiCalls,
        usage: 13, // run.usage,
        status: "completed" // run.status,
    };

    const task = {
        description: run.task.description,
        steps: run.task.steps,
    };

    return (
        <div className="pt-12 mx-auto px-4 w-full lg:w-2/3">
            <RunDetails runStats={runStats} task={task} />
        </div>
    );
};

Run.layout = (page) => <InspectLayout children={page} title="Run" />;

export default Run;
