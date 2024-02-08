@extends('layouts.main')

@section('title', 'Create Agent')

@section('content')

<div class="my-8 mx-auto max-w-xl">
    <div class="font-bold text-xl">{{ $agent->name }}</div>
    <div class="mt-1 text-sm text-gray">{{ $agent->description }}</div>

    <!-- button to add a plugin -->
    <x-button variant="outline" size="lg" class="mt-8">
        Add Plugin
    </x-button>
</div>

@endsection
