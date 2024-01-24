@props(['step'])

    <x-card class="relative">
        <x-card-header>
            <x-card-title>{{ $step->order }}. {{ $step->name }}</x-card-title>
            <x-card-description>{{ $step->description }}</x-card-description>
        </x-card-header>
        <x-card-content>
            <div class="-mt-2 text-sm">
                <x-badge variant="secondary">{{ $step->category }}</x-badge>
                @if($step->params)
                    <div class="mt-4">
                        @php
                            $paramsPretty = json_encode(json_decode($step->params), JSON_PRETTY_PRINT |
                            JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
                        @endphp
                        <pre class="bg-gray-100 rounded p-2 text-xs font-mono">{{ $paramsPretty }}</pre>
                    </div>
                @endif
            </div>
        </x-card-content>
    </x-card>
