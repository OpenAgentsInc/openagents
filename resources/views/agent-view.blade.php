<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Agent Details</title>
    <link href="{{ asset('css/app.css') }}" rel="stylesheet">
    <link rel="stylesheet" type="text/css" href="{{ asset('vendor/litegraph/litegraph.css') }}">
    <script type="text/javascript" src="{{ asset('vendor/litegraph/litegraph.js') }}"></script>
</head>
<body class="bg-gray-100">
    <div class="container mx-auto p-4">
        <h1 class="text-2xl font-bold mb-4">Agent Details: {{ $agent->name }}</h1>

        <div class="bg-white p-4 shadow rounded-lg mb-4">
            <h2 class="text-xl font-semibold mb-2">Owner: {{ $owner }}</h2>
            <p>{{ $agent->description }}</p>
            <p><strong>Balance:</strong> {{ $agent->balance }}</p>
        </div>

        <div class="bg-white p-4 shadow rounded-lg mb-4">
            <h2 class="text-xl font-semibold mb-2">Tasks and Steps</h2>
            <canvas id="mycanvas" width="1024" height="720" style="border: 1px solid"></canvas>
        </div>
    </div>

    <script>
        var graph = new LGraph();
        var canvas = new LGraphCanvas("#mycanvas", graph);

        @foreach($agent->tasks as $task)
            var taskNode = LiteGraph.createNode("basic/const");
            taskNode.title = "{{ $task->name }}";
            taskNode.pos = [Math.random() * 1024, Math.random() * 720];
            graph.add(taskNode);

            @foreach($task->steps as $step)
                var stepNode = LiteGraph.createNode("basic/watch");
                stepNode.title = "{{ $step->name }}";
                stepNode.pos = [Math.random() * 1024, Math.random() * 720];
                graph.add(stepNode);
                taskNode.connect(0, stepNode, 0);
            @endforeach
        @endforeach

        graph.start();
    </script>
</body>
</html>
