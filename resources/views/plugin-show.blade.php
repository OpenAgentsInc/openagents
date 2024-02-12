@extends('layouts.main')

@section('title', 'Plugins')

@section('content')

<div class="md:flex md:items-center md:justify-between">
    <div class="min-w-0 flex-1 flex items-center">
        <h2 class="text-2xl font-bold leading-7 dark:text-white sm:truncate sm:text-3xl sm:tracking-tight mr-4">
            {{ $plugin->name }}
        </h2>
        <p>{{ $plugin->fee }} sats</p>
    </div>
</div>

<div class="prose dark:prose-invert">
    <p>{{ $plugin->description }}</p>
</div>

<div class="mt-8 flex gap-6">
    <div class="flex-1 flex flex-col">
        <x-card>
            <x-card-header>
                <x-card-title>Input</x-card-title>
            </x-card-header>
            <x-card-content>
                <form hx-post="{{ route('plugins.call') }}" hx-target="#plugin-output"
                    hx-swap="innerHTML" class="flex items-end gap-4">
                    @csrf
                    <!-- create a hidden input with the plugin id -->
                    <input type="hidden" name="plugin_id" value="{{ $plugin->id }}" />

                    <x-input type="text" id="input" name="input" placeholder="Enter test data" class="flex-1" />
                    <x-button type="submit">
                        Test
                    </x-button>
                </form>
            </x-card-content>
        </x-card>
    </div>

    <div class="flex-1 flex flex-col">
        <x-card>
            <x-card-header>
                <x-card-title>Output</x-card-title>
            </x-card-header>
            <x-card-content>
                <div id="plugin-output" class="h-full rounded-md p-4">
                    <!-- Plugin output will be displayed here -->
                </div>
            </x-card-content>
        </x-card>
    </div>
</div>

@endsection
