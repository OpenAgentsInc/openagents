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
    <h1>Edit Profile</h1>
    <div hx-target="this">
        @fragment("edit-form")
            @if(isset($successMessage))
                <div class="bg-green-100 text-green-600 rounded p-2 mb-2">
                    {{ $successMessage }}
                </div>
            @endif
            <form id="edit-profile-form" hx-post="/update-profile" hx-target="#edit-profile-form" hx-swap="outerHTML"
                hx-indicator="#indicator">
                @csrf
                <div>
                    <label for="name">Name:</label>
                    <input type="text" id="name" name="name" required>
                </div>
                <div>
                    <label for="email">Email:</label>
                    <input type="email" id="email" name="email" autocomplete="email" required>
                </div>
                <div>
                    <label for="password">New Password:</label>
                    <input type="password" id="password" name="password" autocomplete="new-password">
                </div>
                <div>
                    <label for="password_confirmation">Confirm Password:</label>
                    <input type="password" id="password_confirmation" name="password_confirmation"
                        autocomplete="new-password">
                </div>
                <button type="submit">Update Profile</button>
            </form>
        @endfragment
    </div>
    <span class="htmx-indicator" id="indicator">Loading...</span>
</body>

</html>
