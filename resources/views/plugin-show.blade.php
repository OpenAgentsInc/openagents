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
        <button type="button"
            class="inline-flex items-center rounded-md bg-indigo-400 dark:bg-white/10 px-3 py-2 text-sm font-semibold text-white shadow-sm hover:bg-white/20">Edit</button>
        <button type="button"
            class="ml-3 inline-flex items-center rounded-md bg-indigo-500 px-3 py-2 text-sm font-semibold text-white shadow-sm hover:bg-indigo-400 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-500">Publish</button>
    </div>
</div>

<div class="prose dark:prose-invert">
    <p>{{ $plugin->description }}</p>
    <p>{{ $plugin->fee }}</p>
    <p>{{ $plugin->wasm_url }}</p>
</div>
@endsection
