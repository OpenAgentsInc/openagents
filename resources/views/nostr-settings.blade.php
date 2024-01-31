@extends('layouts.main')

@vite('resources/js/nostr/index.js')

@section('title', 'Nostr Settings')

@section('content')

<div>

<form x-data="setrelays" @submit.prevent="submitForm()">
    <x-input id="newrelays" x-model="newrelays" placeholder="example: wss://relay.com,wss://another.relay.io or leave blank for default" />
    <x-button>save</x-button>
</form>

</div>

@endsection
