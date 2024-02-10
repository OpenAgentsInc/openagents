<h1>all agents!</h1>

<!-- loop through $agents and show each name -->

@foreach($agents as $agent)
    <p>{{ $agent->name }}</p>
@endforeach
