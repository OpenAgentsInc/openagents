{{ $agents->count() }} agents
@foreach($agents as $agent)
    {{ $agent->name }}
@endforeach

{{ $tasks->count() }} tasks
@foreach($tasks as $task)
    {{ $task->prompt }}
@endforeach

{{ $steps->count() }} steps
@foreach($steps as $step)
    @php
    $inputDecoded = json_decode($step->input);
    $outputDecoded = json_decode($step->output);
    @endphp

    @if($inputDecoded)
        {{-- Display each property of input JSON separately --}}
        Type: {{ $inputDecoded->type ?? '' }}
        Model: {{ $inputDecoded->model ?? '' }}
        Instruction: {{ $inputDecoded->instruction ?? '' }}
    @endif

    @if($outputDecoded)
        {{-- Display each property of output JSON separately --}}
        Response: {{ $outputDecoded->response ?? '' }}
        Tokens Used: {{ $outputDecoded->tokens_used ?? '' }}
    @endif
@endforeach
