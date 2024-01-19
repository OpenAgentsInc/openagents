@extends('layouts.main')

@section('title', 'Plugins')

@section('content')
<x-plugin-grid :plugins="$plugins" />

<x-plugin-upload-form />
@endsection
