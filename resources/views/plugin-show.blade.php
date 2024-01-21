@extends('layouts.main')

@section('title', 'Plugins')

@section('content')

<div class="md:flex md:items-center md:justify-between">
    <div class="min-w-0 flex-1">
        <h2 class="text-2xl font-bold leading-7 dark:text-white sm:truncate sm:text-3xl sm:tracking-tight">
            {{ $plugin->name }}
        </h2>
    </div>
    <div class="mt-4 flex md:ml-4 md:mt-0">
        <x-button variant="secondary" class="mr-2">
            Edit
        </x-button>

        <x-button variant="primary">
            Publish
        </x-button>
    </div>
</div>

<div class="prose dark:prose-invert">
    <p>{{ $plugin->description }}</p>
    <p>{{ $plugin->fee }}</p>
    <p>{{ $plugin->wasm_url }}</p>
</div>
@endsection
