@vite(['resources/css/app.css'])

    <div class="max-w-4xl mx-auto py-6">
        <div class="bg-white shadow overflow-hidden sm:rounded-lg mb-4">
            <div class="px-4 py-5 sm:px-6">
                <h3 class="text-lg leading-6 font-medium text-grey-900">Step Details</h3>
            </div>
            <div class="border-t border-grey-200">
                <div class="px-4 py-5 sm:px-6">
                    @php
                        $inputDecoded = json_decode($input);
                        $outputDecoded = json_decode($output);
                    @endphp

                    @if($inputDecoded)
                        <p class="text-sm text-grey-500">
                            Type: <span
                                class="font-medium text-grey-700">{{ $inputDecoded->type ?? 'N/A' }}</span>
                        </p>
                        <p class="text-sm text-grey-500">
                            Model: <span
                                class="font-medium text-grey-700">{{ $inputDecoded->model ?? 'N/A' }}</span>
                        </p>
                        <p class="text-sm text-grey-500">
                            Instruction: <span
                                class="font-medium text-grey-700">{{ $inputDecoded->instruction ?? 'N/A' }}</span>
                        </p>
                    @endif

                    @if($outputDecoded)
                        <p class="text-sm text-grey-500">
                            Response:
                            <pre
                                class="font-medium text-grey-700">{{ json_encode($outputDecoded->response, JSON_PRETTY_PRINT) }}</pre>
                        </p>
                        @if(is_object($outputDecoded) && property_exists($outputDecoded, 'usage'))
                        <p class="text-sm text-grey-500">
                            Tokens Used:
                            <pre
                                class="font-medium text-grey-700">{{ json_encode($outputDecoded->usage, JSON_PRETTY_PRINT) }}</pre>
                        </p>
                        @endif
                    @endif
                </div>
            </div>
        </div>
    </div>
