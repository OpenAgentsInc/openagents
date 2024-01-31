@extends('layouts.main')

@vite('resources/js/nostr/index.js')

@section('title', 'Plugins')

@section('content')

<div class="flex gap-3">
<div class="mr-auto">
<a href="/nostr/plugins/create">
    <x-button class="min-w-[124px]">
        Create Plugin
    </x-button>
</a>
</div>

<div class="ml-auto">
    <form x-data="searchplugins()" @submit.prevent="submitForm">
        <x-input class="w-full" x-model="query" placeholder="Search..." />
    </form>
</div>
</div>

<div>

<div class="inline-flex gap-3 w-full">
<form @submit.prevent class="w-full">
<select id="changepluginssource_select" class="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm transition-colors file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50">
<option default value="*">All relays</option>
    <option value="wss://nostr.wine">wss://nostr.wine</option>
    <option disabled></option>
    <option value=":settings">Open Nostr Settings</option>
    </select>
</form>
<!--<x-button variant="outline" size="icon" ><svg class="h-5 w-5" xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-settings"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/></svg></x-button>-->
</div>

</div>

<div id="plugin-grid-wrapper">
    <h1 class="text-2xl font-bold mb-4 text-center">Plugins</h1>
    <div x-data="window.plugins" id="plugin-grid" class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 mb-6">


            <template x-for="plugin in plugins" :key="plugin.id">

            <a x-bind:href="'/nostr/plugin/'+plugin.author+'/'+plugin.slug" class="no-underline text-black dark:text-white">

                <x-card class="relative">
                    <div class="absolute top-0 right-0 mt-2 mr-2">
    <div class="relative bg-elevation3 rounded-md font-sans inline-flex items-center justify-center text-grey-500 dark:text-grey-300 text-lg">
        <svg class="w-5 h-5 pr-1" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M13 10V3L4 14h7v7l9-11h-7z" />
        </svg>
        <span x-html="window.formatSats(Math.round(plugin.fee*10000000))"></span>
    </div>
                    </div>
                    <x-card-header>
                        <x-card-title x-html="plugin.title"></x-card-title>
                        <x-card-description x-html="plugin.description"></x-card-description>
                    </x-card-header>
                    <x-card-content>
                        <p class="text-sm text-grey-500 dark:text-grey-400">Created: <span x-data="{date: new Date(plugin.published)}" x-text="date.toLocaleString('en-US', { month: 'short', day: 'numeric', year: 'numeric' })"></span></p>
                    </x-card-content>
                </x-card>

            </a>

            </template>



            <!--<p class="col-span-full">No plugins available.</p>-->
    </div>
</div>
@endsection


