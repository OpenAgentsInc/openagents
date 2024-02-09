@extends('layouts.main')

@section('title', 'Build Agent')

@section('content')

<div class="my-8 mx-auto max-w-xl">
    <div class="font-bold text-xl">{{ $agent->name }}</div>
    <div class="mt-1 text-sm text-gray">{{ $agent->description }}</div>

    <!-- button to add a plugin -->
    <x-button id="toggleButton" variant="outline" size="lg" class="mt-8">
        Add Plugin
    </x-button>

    <!-- Element to show/hide -->
    <div id="toggleElement" class="mt-4 hidden">
        <p>Select plugin</p>
        <x-plugin-grid :plugins="$plugins" />
    </div>
</div>

@endsection

@push('scripts')
    <script>
        document.addEventListener('DOMContentLoaded', function () {
            var toggleButton = document.getElementById('toggleButton');
            var toggleElement = document.getElementById('toggleElement');

            toggleButton.addEventListener('click', function () {
                if (toggleElement.classList.contains('hidden')) {
                    toggleElement.classList.remove('hidden');
                } else {
                    toggleElement.classList.add('hidden');
                }
            });
        });

    </script>
@endpush
