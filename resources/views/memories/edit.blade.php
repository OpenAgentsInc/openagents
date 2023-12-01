@extends('layouts.app')

@section('content')
    <form method="POST" action="{{ route('memories.update', $memory->id) }}">
        @csrf
        @method('PUT')

        <label for="title">Title:</label>
        <input type="text" name="title" id="title" value="{{ $memory->title }}">

        <label for="description">Description:</label>
        <textarea name="description" id="description">{{ $memory->description }}</textarea>

        <button type="submit">Update Memory</button>
    </form>
@endsection