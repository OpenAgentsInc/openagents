import { Run as RunType } from '@/Components/RunTable'
import { Button } from '@/Components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/Components/ui/card'
import InspectLayout from '@/Layouts/InspectLayout'
import { usePage } from '@inertiajs/react'

interface Task {
    agent_id: number
    created_at: string
    description: string
    id: number
    output: object | null
    prompt: string
    steps: any[]
    updated_at: string
}

function RunCard({ run, children }: { run: RunType; children?: React.ReactNode }) {
    const { agent_id, created_at, amount, description, id, output, status, task_id } = run;

    return (
        <Card>
            <CardHeader>
                <CardTitle>Run details</CardTitle>
                <CardDescription>What we got</CardDescription>
            </CardHeader>
            <CardContent>
                <div className="flex flex-col">
                    <h2 className="text-lg font-bold">Run #{id}</h2>
                    <div>Agent ID: {agent_id}</div>
                    <div>Amount: {amount}</div>
                    <div>Created at: {created_at}</div>
                    <div>Description: {description}</div>
                    <div>Output: {JSON.stringify(output)}</div>
                    <div>Status: {status}</div>
                    <div>Task ID: {task_id}</div>
                    {children}
                </div>
            </CardContent>
        </Card>
    );
}

function TaskCard({ task }: { task: Task }) {
    return (
        <Card>
            <CardHeader>
                <CardTitle>Task #{task.id}</CardTitle>
                <CardDescription>{task.prompt}</CardDescription>
            </CardHeader>
            <CardContent>
                <div>{task.description}</div>
                <StepsTable steps={task.steps} />
            </CardContent>
        </Card>
    )
}

function StepsTable({ steps }: { steps: any[] }) {
    return (
        <table className="table">
            <thead>
                <tr>
                    <th>Step</th>
                    <th>Prompt</th>
                    <th>Description</th>
                    <th>Command</th>
                    <th>Output</th>
                </tr>
            </thead>
            <tbody>
                {steps.map((step, index) => (
                    <tr key={index}>
                        <td>{index + 1}</td>
                        <td>{step.prompt}</td>
                        <td>{step.description}</td>
                        <td>{step.command}</td>
                        <td>{step.output}</td>
                    </tr>
                ))}
            </tbody>
        </table>
    )
}

function Run() {
    const props = usePage().props
    const run = props.run as RunType
    const task = run.task as Task
    console.log(task)
    const { agent_id, created_at, amount, description, id, output, status, task_id } = run
    const handleInspectTask = () => {
        console.log('open a modal with task details')
    }

    return (
        <div className="pt-12 mx-auto px-4 w-full lg:w-2/3">
            <RunCard run={run}>
                <Button onClick={handleInspectTask}>Inspect Task</Button>
            </RunCard>
        </div>
    )
}

Run.layout = (page) => <InspectLayout children={page} title="Run" />

export default Run
