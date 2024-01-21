@extends('layouts.main')

@section('title', 'Plugins')

@section('content')
<div class="prose dark:prose-invert">
    {{ $plugin->name }}
    {{ $plugin->description }}
    {{ $plugin->fee }}
    {{ $plugin->wasm_url }}
</div>
@endsection
