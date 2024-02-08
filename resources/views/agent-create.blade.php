@extends('layouts.main')

@section('title', 'Create Agent')

@section('content')

<div class="my-8 mx-auto max-w-xl">
    <div class="font-bold text-xl">Create Agent</div>
    <div class="mt-1 text-sm text-gray">Let's fucking go</div>
    <form id="upload-agent" method="POST" action="{{ route('agents.store') }}"
        enctype="multipart/form-data" class="space-y-6">
        @csrf
        <div>
            <x-label for="name">Name</x-label>
            <x-input id="name" name="name" placeholder="Agent Name" autofocus />
        </div>
        <div>
            <x-label for="description">Description</x-label>
            <x-textarea id="description" name="description" placeholder="Agent Description">
            </x-textarea>
        </div>
        <div class="flex justify-center">
            <x-button variant="outline" size="lg" type="submit">
                Create
            </x-button>
        </div>
    </form>
</div>

@endsection
