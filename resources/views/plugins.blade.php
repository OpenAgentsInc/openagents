@extends('layouts.main')

@section('title', 'Plugins')

@section('content')
<a href="/plugins/create" class="inline-block">
    <x-button>
        Create Plugin
    </x-button>
</a>

<div id="plugin-grid-wrapper">
    <x-plugin-grid :plugins="$plugins" />
</div>
@endsection
