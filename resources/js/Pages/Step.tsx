import InspectLayout from '@/Layouts/InspectLayout';
import { Link, usePage } from '@inertiajs/react';
import { Button } from '@/Components/ui/button';
import { Run } from '@/Components/RunTable';
import { Step as RunStep } from '@/Components/RunDetails';

const Step = () => {
    const { props } = usePage();
    console.log(props)
    // const step = props.step as RunStep;
    // console.log(step)
    return null
    return (
        <div className="pt-8 mx-auto px-4 w-full lg:w-3/4">
            <Link href={`/run/${props.run.id}`} className="px-8">
                <Button variant="outline">
                    &larr; Back to step
                </Button>
            </Link>
        </div>
    );
};

Step.layout = (page) => <InspectLayout children={page} title="Run" />;

export default Step;
