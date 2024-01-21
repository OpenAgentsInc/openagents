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
</div>

<div class="mt-8 p-6 bg-grey-100 rounded-lg shadow-lg">
    <h3 class="text-xl font-semibold mb-4">Test Plugin</h3>
    <form action="{{ route('plugins.test', $plugin->id) }}" method="POST">
        @csrf
        <div class="mb-4">
            <label for="test-input" class="block text-md font-medium mb-2">Input for Testing</label>
            <input type="text" id="test-input" name="test_input"
                class="px-3 py-2 w-full rounded-md border-gray-300 shadow-sm focus:border-teal-vivid-300 focus:ring focus:ring-teal-vivid-200 focus:ring-opacity-50"
                placeholder="Enter test data">
        </div>
        <x-button type="submit">
            Test Plugin
        </x-button>
    </form>
</div>

@endsection
