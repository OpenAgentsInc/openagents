@extends('layouts.main')

@section('title', 'Create Agent')

@section('content')

<div class="grid grid-cols-3 gap-4">
    @foreach($agents as $agent)
        <a href="{{ route('agent.build', $agent) }}">
            <div class="border-2 border-offblack rounded-[16px] p-4">
                <p>{{ $agent->name }}</p>
            </div>
        </a>
    @endforeach
</div>

@endsection
