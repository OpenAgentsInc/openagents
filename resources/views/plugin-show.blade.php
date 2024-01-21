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
</div>

<div class="prose dark:prose-invert">
    <p>{{ $plugin->description }}</p>
</div>

<div class="mt-8 flex gap-6">
    <div class="flex-1 flex flex-col">
        <h3 class="text-xl font-semibold mb-4 px-6">Input</h3>
        <div class="flex-1 p-6 bg-grey-100 rounded-lg shadow-lg">
            <form action="{{ route('plugins.test', $plugin->id) }}" method="POST"
                class="flex flex-col h-full">
                @csrf
                <input type="text" id="test-input" name="test_input"
                    class="mb-4 px-3 py-2 w-full rounded-md border-gray-300 shadow-sm focus:border-teal-vivid-300 focus:ring focus:ring-teal-vivid-200 focus:ring-opacity-50"
                    placeholder="Enter test data">
                <x-button type="submit" class="mt-auto">
                    Test
                </x-button>
            </form>
        </div>
    </div>

    <div class="flex-1 flex flex-col">
        <h3 class="text-xl font-semibold mb-4 px-6">Output</h3>
        <div class="flex-1 p-6 bg-grey-200 rounded-lg shadow-lg">
            <div id="plugin-output" class="h-full bg-white rounded-md p-4">
                <!-- Plugin output will be displayed here -->
            </div>
        </div>
    </div>
</div>

@endsection
