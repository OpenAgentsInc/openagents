@extends('layouts.main')

@vite('resources/js/nostr/index.js')

@section('title', 'Plugins')

@section('content')

<div id="wrapper" x-data="{ data: null }" x-init="async () => { data = await window.plugin('{{ $pubkey }}', '{{ $title }}'); }">

<div class="md:flex md:items-center md:justify-between">
    <div class="min-w-0 flex-1 flex items-center">
        <h2 x-html="data ? data.title : 'Loading...'" class="text-2xl font-bold leading-7 dark:text-white sm:truncate sm:text-3xl sm:tracking-tight mr-4">
        </h2>
    <div class='text-lg relative bg-elevation3 rounded-md font-sans inline-flex items-center justify-center text-grey-500 dark:text-grey-300'>
        <svg class="w-5 h-5 pr-1" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M13 10V3L4 14h7v7l9-11h-7z" />
        </svg>
        <span x-html="window.formatSats(Math.round((data ? data.fee : 0)*10000000))"></span>
    </div>
    </div>
</div>

<div x-html="data ? data.description : 'Loading...'" class="prose dark:prose-invert">
    <p>description</p>
</div>

<div class="mt-8 flex gap-6">
    <div class="flex-1 flex flex-col">
        <x-card>
            <x-card-header>
                <x-card-title>Input</x-card-title>
            </x-card-header>
            <x-card-content>
                <form hx-target="#plugin-output"
                    hx-swap="innerHTML" class="flex items-end gap-4">
                    @csrf
                    <!-- create a hidden input with the plugin id -->
                    <input type="hidden" name="plugin_id" value="pluginid" />

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

</div>
@endsection
