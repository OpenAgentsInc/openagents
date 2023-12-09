import InspectLayout from '@/Layouts/InspectLayout';
import { Link, usePage } from '@inertiajs/react';
import { Button } from '@/Components/ui/button';
import { Step as RunStep } from '@/Components/RunDetails';
import { StepDetails } from '@/Components/StepDetails';

const Step = () => {
    const { props } = usePage();
    const step = props.step as RunStep;
    // console.log(step)
    return (
        <div className="pt-8 mx-auto px-4 w-full lg:w-3/4">
            <Link href={`/run/${step.run_id}`} className="px-8">
                <Button variant="outline">
                    &larr; Back to run
                </Button>
            </Link>
            <StepDetails step={step} />
        </div>
    );
};

Step.layout = (page) => <InspectLayout children={page} title="Run" />;

export default Step;
