@extends('layouts.main')

@section('title', 'OpenAgents Blog')

@section('content')

<div class="prose dark:prose-invert mx-auto">
    {!! $htmlContent !!}
</div>

@endsection
