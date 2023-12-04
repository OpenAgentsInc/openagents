import { RunTable } from '@/Components/RunTable'
import InspectLayout from '@/Layouts/InspectLayout'

function Inspect() {
    return (
        <div className="mt-32 mx-auto h-screen w-2/3">
            <RunTable />
        </div>
    )
}

Inspect.layout = (page) => <InspectLayout children={page} title="Inspect" />

export default Inspect
