@vite(['resources/css/app.css'])

    <div class="max-w-4xl mx-auto py-6">
        <div class="bg-white shadow overflow-hidden sm:rounded-lg mb-4">
            <div class="px-4 py-5 sm:px-6">
                <h3 class="text-lg leading-6 font-medium text-gray-900">Step Details</h3>
            </div>
            <div class="border-t border-gray-200">
                <div class="px-4 py-5 sm:px-6">
                    @php
                        $inputDecoded = json_decode($input);
                        $outputDecoded = json_decode($output);
                    @endphp

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
                            <pre
                                class="font-medium text-gray-700">{{ json_encode($outputDecoded->response, JSON_PRETTY_PRINT) }}</pre>
                        </p>
                        <p class="text-sm text-gray-500">
                            Tokens Used: <span
                                class="font-medium text-gray-700">{{ $outputDecoded->tokens_used ?? 'N/A' }}</span>
                        </p>
                    @endif
                </div>
            </div>
        </div>
    </div>
