@extends('layouts.main')

@section('title', 'Create Plugin')

@section('content')
<div class="prose dark:prose-invert">
    <div id="plugin-form-wrapper">
        <x-plugin-upload-form />
    </div>
</div>
@endsection
