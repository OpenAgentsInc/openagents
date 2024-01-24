@extends('layouts.main')

@section('title', 'OpenAgents')

@section('content')

<div class="container mx-auto p-4">
    <h1 class="text-2xl font-bold mb-4">Agent Details: {{ $agent->name }}</h1>

    <div class="p-4 shadow rounded-lg mb-4">
        <h2 class="text-xl font-semibold mb-2">Owner: {{ $owner }}</h2>
        <p>{{ $agent->description }}</p>
        <p><strong>Balance:</strong> {{ $agent->balance }}</p>
    </div>

    @foreach($agent->tasks as $task)
        <div class="p-4 shadow rounded-lg mb-4">
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
