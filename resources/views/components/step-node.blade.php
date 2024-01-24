@props(['step'])

    <x-card class="relative">
        <x-card-header>
            <x-card-title>{{ $step->order }}. {{ $step->name }}</x-card-title>
            <x-card-description>{{ $step->description }}</x-card-description>
        </x-card-header>
        <x-card-content>
            <div class="text-sm">
                <span
                    class="inline-block bg-blue-200 text-blue-800 text-xs px-2 rounded-full uppercase font-semibold tracking-wide">
                    {{ $step->category }}
                </span>
                <p><strong>Params:</strong> {{ json_encode($step->params) }}</p>
            </div>
        </x-card-content>
    </x-card>
