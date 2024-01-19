@extends('layouts.main')

@section('title', 'Plugins')

@section('content')
<div id="plugin-grid-wrapper">
    <x-plugin-grid :plugins="$plugins" />
</div>

<div id="plugin-form-wrapper">
    <x-plugin-upload-form />
</div>
@endsection
