<!-- task-runner.blade.php -->

@props(['task'])

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
        </div>

        <div class="flex-1 flex flex-col">
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
    </div>
