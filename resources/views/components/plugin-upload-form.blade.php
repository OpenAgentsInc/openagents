<form id="upload-plugin" hx-post="/plugins" hx-target="#plugin-grid" hx-swap="outerHTML" enctype="multipart/form-data" class="my-12 mx-auto max-w-xl space-y-4">
    <h1 class="text-2xl font-bold mb-4 text-center">Upload Plugin</h1>
    @csrf
    <div>
        <label for="name" class="block text-md font-medium">Name</label>
        <input type="text"
            class="px-3 py-2 mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-indigo-300 focus:ring focus:ring-indigo-200 focus:ring-opacity-50 bg-gray-100 dark:bg-gray-700 dark:border-gray-600 dark:text-white"
            id="name" name="name" placeholder="Plugin Name">
    </div>
    <div>
        <label for="description" class="block text-md font-medium">Description</label>
        <textarea
            class="px-3 py-2 mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-indigo-300 focus:ring focus:ring-indigo-200 focus:ring-opacity-50 bg-gray-100 dark:bg-gray-700 dark:border-gray-600 dark:text-white"
            id="description" name="description" placeholder="Plugin Description"></textarea>
    </div>
    <div>
        <label for="wasm_url" class="block text-md font-medium">Wasm URL</label>
        <input type="text"
            class="px-3 py-2 mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-indigo-300 focus:ring focus:ring-indigo-200 focus:ring-opacity-50 bg-gray-100 dark:bg-gray-700 dark:border-gray-600 dark:text-white"
            id="wasm_url" name="wasm_url" placeholder="Plugin Wasm URL">
    </div>

    <div>
        <label for="fee" class="block text-md font-medium">Fee</label>
        <input type="range" class="mt-1 w-full rounded-md focus:ring-indigo-200 focus:ring-opacity-50" id="fee"
            name="fee" min="0" max="100" value="0">
        <span id="fee-value" class="text-md font-medium">0</span> sats
    </div>

    <button type="submit"
        class="w-full px-4 py-2 bg-blue-500 text-white font-bold rounded hover:bg-blue-600 focus:outline-none focus:ring-2 focus:ring-blue-300 focus:ring-opacity-50">Upload
        Plugin</button>
</form>

<script>
    // add an event listener to the slider
    document.getElementById("fee").addEventListener("input", function () {
        // get the value of the slider
        var fee = document.getElementById("fee").value;
        // set the text of the span to the value of the slider
        document.getElementById("fee-value").innerHTML = fee;
    });

</script>
