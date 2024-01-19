@extends('layouts.main')

@section('title', 'Plugins')

@section('content')

<h1 class="text-2xl font-bold mb-4 text-center">Plugins</h1>
<div class="mb-6">
    @forelse($plugins as $plugin)
        <div class="mb-2">
            <h2 class="text-lg font-semibold">{{ $plugin->name }}</h2>
            <!-- Add more plugin details here if needed -->
        </div>
    @empty
        <p>No plugins available.</p>
    @endforelse
</div>

<h1 class="text-2xl font-bold mb-4 text-center">Upload Plugin</h1>
<x-plugin-upload-form />
@endsection
