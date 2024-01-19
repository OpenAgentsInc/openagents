@extends('layouts.main')

@section('title', 'Plugins')

@section('content')

<h1 class="text-2xl font-bold mb-4 text-center">Plugins</h1>
<x-plugin-grid :plugins="$plugins" />


<h1 class="text-2xl font-bold mb-4 text-center">Upload Plugin</h1>
<x-plugin-upload-form />
@endsection
