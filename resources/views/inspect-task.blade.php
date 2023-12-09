@vite(['resources/css/app.css'])

<div class="max-w-4xl mx-auto py-6">
    <div class="bg-white shadow overflow-hidden sm:rounded-lg mb-4">
        <div class="px-4 py-5 sm:px-6">
            <h3 class="text-lg leading-6 font-medium text-gray-900">Task: {{ $task->description }}</h3>
        </div>
        <div class="border-t border-gray-200">
            @foreach($steps as $step)
                @php
                    $inputDecoded = json_decode($step->input);
                    $outputDecoded = json_decode($step->output);
                @endphp

                <div class="px-4 py-5 sm:px-6">
                    <h4 class="text-lg font-medium text-blue-600">
                        <a href="{{ route('inspect-step', $step->id) }}">Step {{ $loop->iteration }} - {{ $step->description }}</a>
                    </h4>

                    @if(isset($inputDecoded->type))
                        <p class="text-sm text-gray-500">Type: <span class="font-medium text-gray-700">{{ $inputDecoded->type }}</span></p>
                    @endif
                    @if(isset($inputDecoded->model))
                        <p class="text-sm text-gray-500">Model: <span class="font-medium text-gray-700">{{ $inputDecoded->model }}</span></p>
                    @endif
                    @if(isset($inputDecoded->instruction))
                        <p class="text-sm text-gray-500">Instruction: <span class="font-medium text-gray-700">{{ $inputDecoded->instruction }}</span></p>
                    @endif

                    @if(isset($outputDecoded->tokens_used))
                        <p class="text-sm text-gray-500">Duration: <span class="font-medium text-gray-700">{{ $outputDecoded->tokens_used }}ms</span></p>
                    @endif
                </div>
            @endforeach
        </div>
    </div>
</div>
