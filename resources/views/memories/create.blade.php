@extends('layouts.app')

@section('content')
    <form method="POST" action="{{ route('memories.store') }}">
        @csrf
        <label for="title">Title:</label>
        <input type="text" name="title" id="title" required>
        <label for="description">Description:</label>
        <textarea name="description" id="description" required></textarea>
        <label for="date">Date:</label>
        <input type="date" name="date" id="date" required>
        <button type="submit">Create Memory</button>
    </form>
@endsection