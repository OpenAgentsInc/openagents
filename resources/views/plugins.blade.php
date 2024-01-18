<!DOCTYPE html>
<html lang="en">

<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Edit Profile</title>
    <script src="https://unpkg.com/htmx.org@1.9.10"
        integrity="sha384-D1Kt99CQMDuVetoL1lrYwg5t+9QdHe7NLX/SoJYkXDFfX37iInKRy5xLSi8nO7UC" crossorigin="anonymous">
    </script>

</head>

<body>
    <h1>Upload Plugin</h1>

    <form id="upload-plugin" hx-post="/plugins" enctype="multipart/form-data">
        @csrf
        <div class="form-group">
            <label for="name">Name</label>
            <input type="text" class="form-control" id="name" name="name" placeholder="Plugin Name">
        </div>
        <div class="form-group">
            <label for="description">Description</label>
            <textarea class="form-control" id="description" name="description"
                placeholder="Plugin Description"></textarea>
        </div>
        <div class="form-group">
            <label for="wasm_url">Wasm URL</label>
            <input type="text" class="form-control" id="wasm_url" name="wasm_url" placeholder="Plugin Wasm URL">
        </div>

        <div class="form-group">
            <label for="fee">Fee</label>
            <input type="range" class="form-control" id="fee" name="fee" min="0" max="100" value="0">
            <span id="fee-value">0</span> sats
        </div>

        <button type="submit" class="btn btn-primary">Upload Plugin</button>
    </form>

</body>

<script>
    // add an event listener to the slider
    document.getElementById("fee").addEventListener("input", function () {
        // get the value of the slider
        var fee = document.getElementById("fee").value;
        // set the text of the span to the value of the slider
        document.getElementById("fee-value").innerHTML = fee;
    });

</script>

</html>
