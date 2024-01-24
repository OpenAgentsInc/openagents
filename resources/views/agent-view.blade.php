@extends('layouts.main')

@section('title', 'OpenAgents')

@section('content')

<div class="container mx-auto p-4">
    <x-card class="mb-4">
        <x-card-header>
            <x-card-title>{{ $agent->name }}</x-card-title>
            <x-card-description>{{ $agent->description }}</x-card-description>
        </x-card-header>
        <x-card-content>
            <p><strong>Owner:</strong> {{ $owner }}</p>
            <p><strong>Balance:</strong> {{ $agent->balance }}</p>
        </x-card-content>
    </x-card>

    @foreach($agent->tasks as $task)
        <div class="p-4 mb-4">
            <h2 class="text-xl font-semibold mb-2">{{ $task->name }}</h2>
            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 mb-6">
                @forelse($task->steps->sortBy('order') as $step)
                    <x-step-node :step="$step" />
                @empty
                    <p class="col-span-full">No steps available for this task.</p>
                @endforelse
            </div>
        </div>
    @endforeach
</div>

@endsection
