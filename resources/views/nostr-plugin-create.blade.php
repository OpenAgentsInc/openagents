@extends('layouts.main')

@vite('resources/js/nostr/index.js')

@section('title', 'Create Plugin')

@section('content')

<div id="plugin-form-wrapper">
    <x-card class="my-8 mx-auto max-w-xl">
        <x-card-header>
            <x-card-title>Create Plugin</x-card-title>
            <x-card-description>
                Make a new agent plugin from an Extism .wasm file.<br/>
                Make sure you have a nostr browser extention installed to broadcast the plugin to the nostr network.
            </x-card-description>
        </x-card-header>
        <x-card-content>
        <div x-data="createplugin()">
            <form @submit.prevent="submitForm" class="space-y-4">
                <!-- @csrf -->
                <div>
                    <x-label for="name">Name</x-label>
                    <x-input x-model="title" name="name" placeholder="Plugin Name" class="mt-1 block w-full" />
                </div>
                <div class="mt-2">
                    <x-label for="description">Description</x-label>
                    <x-textarea x-model="description" name="description" class="!outline-none mt-1"
                        placeholder="Plugin Description">
                    </x-textarea>
                </div>
                <div class="mt-2">
                    <x-label for="wasm_url">Wasm URL</x-label>
                    <x-input x-model="wasm_url" name="wasm_url" placeholder="Plugin Wasm URL" class="mt-1 block w-full" />
                </div>
                <div class="mt-2">
                    <x-label for="fee">Fee (sats)</x-label>
                    <x-input type="number" name="fee" x-model="fee" value="0" placeholder="Optional Plugin Fee (in Sats), 0 = free" class="mt-1 block w-full" />
                </div>
                <div class="flex justify-center">
                    <x-button variant="outline" size="lg" type="submit">
                        Create
                    </x-button>
                </div>
            </form>
            <div><span id="createplugin_res" x-data="createplugin_res()" x-html="message"></span></div>
        </div>
        </x-card-content>
    </x-card>
</div>
@endsection

