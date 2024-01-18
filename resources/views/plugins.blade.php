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
            <label for="plugin-name">Plugin Name</label>
            <input type="text" class="form-control" id="plugin-name" name="plugin-name" placeholder="Plugin Name">
        </div>
        <div class="form-group">
            <label for="plugin-description">Plugin Description</label>
            <textarea class="form-control" id="plugin-description" name="plugin-description"
                placeholder="Plugin Description"></textarea>
        </div>
        <div class="form-group">
            <label for="plugin-wasm-url">Plugin Wasm URL</label>
            <input type="text" class="form-control" id="plugin-wasm-url" name="plugin-wasm-url"
                placeholder="Plugin Wasm URL">
        </div>

        <button type="submit" class="btn btn-primary">Upload Plugin</button>
    </form>

</body>

</html>
