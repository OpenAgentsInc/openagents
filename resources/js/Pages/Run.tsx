import InspectLayout from '@/Layouts/InspectLayout'

function Run () {
    return <h1>hi</h1>
}

Run.layout = (page) => <InspectLayout children={page} title="Run" />

export default Run
