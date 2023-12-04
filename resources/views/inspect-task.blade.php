@vite(['resources/css/app.css'])

    <div class="max-w-4xl mx-auto py-6">
        <div class="bg-white shadow overflow-hidden sm:rounded-lg mb-4">
            <div class="px-4 py-5 sm:px-6">
                <h3 class="text-lg leading-6 font-medium text-gray-900">Task: {{ $task->prompt }}</h3>
            </div>
            <div class="border-t border-gray-200">
                @foreach($steps as $step)
                    @php
                        $inputDecoded = json_decode($step->input);
                        $outputDecoded = json_decode($step->output);
                    @endphp

                    <div class="px-4 py-5 sm:px-6">
                    <h4 class="text-lg font-medium text-blue-600">
                        <a href="{{ route('inspect-step', $step->id) }}">Step {{ $loop->iteration }}</a>
                    </h4>
                        @if($inputDecoded)
                            <p class="text-sm text-gray-500">
                                Type: <span
                                    class="font-medium text-gray-700">{{ $inputDecoded->type ?? 'N/A' }}</span>
                            </p>
                            <p class="text-sm text-gray-500">
                                Model: <span
                                    class="font-medium text-gray-700">{{ $inputDecoded->model ?? 'N/A' }}</span>
                            </p>
                            <p class="text-sm text-gray-500">
                                Instruction: <span
                                    class="font-medium text-gray-700">{{ $inputDecoded->instruction ?? 'N/A' }}</span>
                            </p>
                        @endif

                        @if($outputDecoded)
                            <p class="text-sm text-gray-500">
                                Response:
                                @if(is_array($outputDecoded->response))
                                    <pre
                                        class="font-medium text-gray-700">{{ json_encode($outputDecoded->response, JSON_PRETTY_PRINT) }}</pre>
                                @else
                                    <span
                                        class="font-medium text-gray-700">{{ $outputDecoded->response ?? 'N/A' }}</span>
                                @endif
                            </p>
                            <p class="text-sm text-gray-500">
                                Tokens Used: <span
                                    class="font-medium text-gray-700">{{ $outputDecoded->tokens_used ?? 'N/A' }}</span>
                            </p>
                        @endif
                    </div>
                @endforeach
            </div>
        </div>
    </div>
