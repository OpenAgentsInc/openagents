@extends('layouts.main')

@section('title', 'Plugins')

@section('content')
<div class="prose dark:prose-invert">
    <a href="/plugins/create" class="inline-block">
        <x-button variant="primary">
            Create Plugin
        </x-button>
    </a>

    <div id="plugin-grid-wrapper">
        <x-plugin-grid :plugins="$plugins" />
    </div>
</div>
@endsection
