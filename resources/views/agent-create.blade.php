@extends('layouts.main')

@section('title', 'Create Agent')

@section('content')

<div class="my-8 mx-auto w-[480px] p-[32px] border-2 border-offblack rounded-[16px]">
    <div class="font-bold text-xl">Create Agent</div>
    <div class="mt-1 text-sm text-gray">First the basics</div>
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
            <x-button variant="default" size="lg" type="submit" class="w-full mt-[22px]">
                Create
            </x-button>
        </div>
    </form>
</div>

@endsection
