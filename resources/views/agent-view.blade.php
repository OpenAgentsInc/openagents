@extends('layouts.main')

@section('title', 'OpenAgents')

@section('content')

<div class="container mx-auto p-4">
    <x-card class="mb-4 relative">
        <div class="absolute top-0 right-0 mt-4 mr-2 text-right mr-4">
            <x-bitcoin-amount :amount="$agent->balance" />
            <p class="italic text-sm mt-1">Owned by {{ $owner }}</p>
        </div>
        <x-card-header>
            <x-card-title>{{ $agent->name }}</x-card-title>
            <x-card-description>{{ $agent->description }}</x-card-description>
        </x-card-header>
    </x-card>

    @foreach($agent->tasks as $task)
        <div class="py-4 mb-4">
            <div class="mb-4">
                <span class="uppercase text-xs opacity-75 tracking-wider">Task</span>
                <h2 class="-mt-2 py-2 text-lg font-bold rounded-t-lg">{{ $task->name }}</h2>
            </div>
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
