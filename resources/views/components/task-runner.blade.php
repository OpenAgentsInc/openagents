@props(['task', 'stepExecutedData' => null])

<div class="flex gap-6">
    <div class="flex-1 flex flex-col">
        <x-card>
            <x-card-header>
                <x-card-title>Input</x-card-title>
            </x-card-header>
            <x-card-content>
                <form
                    hx-post="{{ route('agent.run_task', ['id' => $task->id]) }}"
                    hx-target="#task-output-{{ $task->id }}" hx-swap="innerHTML" class="flex items-end gap-4">
                    @csrf
                    <!-- create a hidden input with the task id -->
                    <input type="hidden" name="task_id" value="{{ $task->id }}" />

                    <x-input type="text" id="input" name="input" placeholder="Enter test data" class="flex-1" />
                    <x-button type="submit">
                        Run Task
                    </x-button>
                </form>
            </x-card-content>
        </x-card>

        <x-card>
            <x-card-header>
                <x-card-title>Output</x-card-title>
            </x-card-header>
            <x-card-content>
                <div id="task-output-{{ $task->id }}" class="h-full rounded-md p-4">
                    <!-- Task output will be displayed here -->
                </div>
            </x-card-content>
        </x-card>
    </div>

    <!-- Display input and output for each StepExecuted -->
    <div class="flex-1 flex flex-col">
        @if($stepExecutedData)
            @foreach($stepExecutedData as $stepExecuted)
                <x-card>
                    <x-card-header>
                        <x-card-title>Step {{ $stepExecuted->step->order }} - {{ $stepExecuted->step->name }}</x-card-title>
                    </x-card-header>
                    <x-card-content>
                        <div class="mt-4">
                            <strong>Input:</strong>
                            <pre class="bg-gray-100 rounded p-2 text-xs font-mono">{{ $stepExecuted->input }}</pre>
                        </div>
                        <div class="mt-4">
                            <strong>Output:</strong>
                            <pre class="bg-gray-100 rounded p-2 text-xs font-mono">{{ $stepExecuted->output }}</pre>
                        </div>
                    </x-card-content>
                </x-card>
            @endforeach
        @endif
    </div>
</div>
