@extends('layouts.main')

@section('title', 'Plugins')

@section('content')

<a wire:navigate href="/plugins/create" class="mb-6 inline-block">
    <x-button>
        Create Plugin
    </x-button>
</a>

<x-plugin-grid :plugins="$plugins" />

@endsection
