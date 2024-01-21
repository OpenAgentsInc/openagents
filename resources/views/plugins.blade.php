@extends('layouts.main')

@section('title', 'Plugins')

@section('content')
<div class="prose dark:prose-invert">

    <a href="/plugins/create">
        <button class="bg-slate-500 hover:bg-slate-700 text-white font-bold py-2 px-4 rounded">
            Create Plugin
        </button>
    </a>

    <div id="plugin-grid-wrapper">
        <x-plugin-grid :plugins="$plugins" />
    </div>
</div>
@endsection
