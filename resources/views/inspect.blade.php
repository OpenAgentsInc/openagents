@vite(['resources/css/app.css', 'resources/js/app.js'])

<div class="max-w-4xl mx-auto py-6">
    <div class="mb-4">
        <h2 class="text-lg font-semibold">{{ $agents->count() }} agents</h2>
        @foreach($agents as $agent)
            <p class="text-gray-600">{{ $agent->name }}</p>
        @endforeach
    </div>

    <div class="mb-4">
        <h2 class="text-lg font-semibold">{{ $tasks->count() }} tasks</h2>
        @foreach($tasks as $task)
            <p class="text-gray-600">{{ $task->prompt }}</p>
        @endforeach
    </div>

    <div>
        <h2 class="text-lg font-semibold">{{ $steps->count() }} steps</h2>
        @foreach($steps as $step)
            @php
            $inputDecoded = json_decode($step->input);
            $outputDecoded = json_decode($step->output);
            @endphp

            <div class="bg-white shadow overflow-hidden sm:rounded-lg mb-4">
                <div class="px-4 py-5 sm:px-6">
                    @if($inputDecoded)
                        <p class="text-sm text-gray-500">
                            Type: <span class="font-medium text-gray-700">{{ $inputDecoded->type ?? 'N/A' }}</span>
                        </p>
                        <p class="text-sm text-gray-500">
                            Model: <span class="font-medium text-gray-700">{{ $inputDecoded->model ?? 'N/A' }}</span>
                        </p>
                        <p class="text-sm text-gray-500">
                            Instruction: <span class="font-medium text-gray-700">{{ $inputDecoded->instruction ?? 'N/A' }}</span>
                        </p>
                    @endif
                </div>
                <div class="border-t border-gray-200">
                    <dl>
                        @if($outputDecoded)
                            <div class="bg-gray-50 px-4 py-5 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6">
                                <dt class="text-sm font-medium text-gray-500">Response</dt>
                                <dd class="mt-1 text-sm text-gray-900 sm:mt-0 sm:col-span-2">{{ $outputDecoded->response ?? 'N/A' }}</dd>
                            </div>
                            <div class="bg-white px-4 py-5 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6">
                                <dt class="text-sm font-medium text-gray-500">Tokens Used</dt>
                                <dd class="mt-1 text-sm text-gray-900 sm:mt-0 sm:col-span-2">{{ $outputDecoded->tokens_used ?? 'N/A' }}</dd>
                            </div>
                        @endif
                    </dl>
                </div>
            </div>
        @endforeach
    </div>
</div>
