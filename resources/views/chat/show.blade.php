@extends('layouts.app')

@section('content')
<div class="container">
    <h1>Chat Thread: {{ $thread->title }}</h1>
    
    <div id="chat-messages" hx-ext="sse" sse-connect="{{ route('chat.stream', ['thread' => $thread->id]) }}">
        <div sse-swap="message">
            <!-- Existing messages will be loaded here -->
            @foreach ($thread->messages as $message)
                @include('partials.message', ['message' => $message])
            @endforeach
        </div>
    </div>

    <form hx-post="{{ route('messages.store') }}" hx-target="#chat-messages" hx-swap="beforeend">
        @csrf
        <input type="hidden" name="thread_id" value="{{ $thread->id }}">
        <div class="form-group">
            <textarea name="content" class="form-control" rows="3" required></textarea>
        </div>
        <button type="submit" class="btn btn-primary">Send</button>
    </form>
</div>
@endsection

@push('scripts')
<script src="https://unpkg.com/htmx-ext-sse@2.2.2/sse.js"></script>
@endpush