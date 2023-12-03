@vite(['resources/css/app.css', 'resources/js/app.js'])

<div class="max-w-4xl mx-auto py-6">
    <h2 class="text-lg font-semibold mb-4">{{ $agents->count() }} agents</h2>

    @foreach($agents as $agent)
        <div class="bg-white shadow overflow-hidden sm:rounded-lg mb-4">
            <div class="px-4 py-5 sm:px-6">
                <h3 class="text-lg leading-6 font-medium text-gray-900">{{ $agent->name }}</h3>
            </div>
            <div class="border-t border-gray-200">
                @foreach($agent->tasks as $task)
                    <div class="px-4 py-5 sm:px-6">
                        <p class="text-sm font-medium text-gray-600">Task: {{ $task->prompt }}</p>

                        @foreach($task->steps as $step)
                            @php
                            $inputDecoded = json_decode($step->input);
                            $outputDecoded = json_decode($step->output);
                            @endphp

                            <div class="mt-4">
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

                                @if($outputDecoded)
                                    <p class="text-sm text-gray-500">
                                        Response: <span class="font-medium text-gray-700">{{ $outputDecoded->response ?? 'N/A' }}</span>
                                    </p>
                                    <p class="text-sm text-gray-500">
                                        Tokens Used: <span class="font-medium text-gray-700">{{ $outputDecoded->tokens_used ?? 'N/A' }}</span>
                                    </p>
                                @endif
                            </div>
                        @endforeach
                    </div>
                @endforeach
            </div>
        </div>
    @endforeach
</div>
