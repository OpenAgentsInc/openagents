<div>
    <h2>Total users: {{ $totalUsers }}</h2>

    <h2 class="mt-12">Last 50 Users and Their Message Counts</h2>
    <table>
        <thead>
        <tr>
            <th>User</th>
            <th>Messages</th>
        </tr>
        </thead>
        <tbody>
        @foreach($users as $user)
            <tr>
                <td>{{ $user->name }}</td>
                <td>{{ $user->messages_count }}</td>
            </tr>
        @endforeach
        </tbody>
    </table>
</div>
