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

        <div class="bg-white p-4 shadow rounded-lg mb-4 w-screen">
            <h2 class="text-xl font-semibold mb-2">Tasks and Steps</h2>
            <canvas id="mycanvas" width="1600" height="800" style="border: 1px solid"></canvas>
        </div>
    </div>

    <script>
// Node constructor class
function StepNode() {
    this.addInput("Prev", "Step");
    this.addOutput("Next", "Step");
    this.properties = {
        stepName: '',
        entryType: '',
        category: '',
        order: 0,
        description: '',
        errorMessage: '',
        successAction: '',
        params: '{}'
    };

    this.widgets_up = true; // Put widgets at the top of the node
    this.addWidget("text", "Name", "", (v) => { this.properties.stepName = v; });
    this.addWidget("text", "Entry Type", "", (v) => { this.properties.entryType = v; });
    this.addWidget("text", "Category", "", (v) => { this.properties.category = v; });
    this.addWidget("number", "Order", 0, (v) => { this.properties.order = v; });
    this.addWidget("text", "Description", "", (v) => { this.properties.description = v; });
    this.addWidget("text", "Error Message", "", (v) => { this.properties.errorMessage = v; });
    this.addWidget("text", "Success Action", "", (v) => { this.properties.successAction = v; });
    this.addWidget("text", "Params (JSON)", "{}", (v) => { this.properties.params = v; });
}

// Name to show
StepNode.title = "Step";

// Function to call when the node is executed
StepNode.prototype.onExecute = function() {
    this.setOutputData(0, this.getInputData(0));
}

// Function to draw additional information on the node
// StepNode.prototype.onDrawForeground = function(ctx) {
//     if(this.flags.collapsed) return;
//     ctx.font = "20px Arial";
//     ctx.textAlign = "center";
//     ctx.fillStyle = "#000";
//     ctx.fillText(this.properties.stepName, this.size[0] * 0.5, this.size[1] * 0.5);
// }

// Register in the system
LiteGraph.registerNodeType("custom/step", StepNode);



var graph = new LGraph();
var canvas = new LGraphCanvas("#mycanvas", graph);

var taskIndex = 0;
@foreach($agent->tasks as $task)
    var previousStepNode = null;
    var xOffset = 40;
    var yOffset = 100 + taskIndex * 300;
    var stepIndex = 0;

    @foreach($task->steps->sortBy('order') as $step)
        console.log("{{ $step->name }}")        
        var stepNode = LiteGraph.createNode("custom/step");
        stepNode.title = "{{ $step->order }}. {{ $step->name }}";
        stepNode.properties.stepName = "{{ $step->name }}";
        stepNode.properties.entryType = "{{ $step->entry_type }}";
        stepNode.properties.category = "{{ $step->category }}";
        stepNode.properties.order = {{ $step->order }};
        stepNode.properties.description = "{{ $step->description }}";
        stepNode.properties.errorMessage = "{{ $step->error_message }}";
        stepNode.properties.successAction = "{{ $step->success_action }}";
        stepNode.properties.params = JSON.stringify(@json($step->params));
        stepNode.pos = [xOffset + stepIndex * 280, yOffset + stepIndex * 20];
        graph.add(stepNode);

        if(previousStepNode != null) {
            previousStepNode.connect(0, stepNode, 0);
        }

        previousStepNode = stepNode;
        stepIndex++;
    @endforeach

    taskIndex++;
@endforeach

graph.start();
</script>


</body>
</html>
