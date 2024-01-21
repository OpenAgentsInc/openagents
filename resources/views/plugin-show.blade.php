@extends('layouts.main')

@section('title', 'Plugins')

@section('content')

<div class="md:flex md:items-center md:justify-between">
    <div class="min-w-0 flex-1 flex items-center">
        <h2 class="text-2xl font-bold leading-7 dark:text-white sm:truncate sm:text-3xl sm:tracking-tight mr-4">
            {{ $plugin->name }}
        </h2>
        <x-bitcoin-amount :amount="$plugin->fee" class="text-lg" />
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
    <!-- <p><a href="{{ $plugin->wasm_url }}" class="text-blue-500 hover:text-blue-700">{{ $plugin->wasm_url }}</a></p> -->
</div>
@endsection
