<!DOCTYPE html>
<html>
<head>
    <title>Payin Status</title>
</head>
<body>
<h1>Payin Status</h1>

<p>User: {{ $payin->user->username }}</p>
<p>Amount: {{ $payin->amount }} msats</p>
<p>Description Hash: {{ $payin->description_hash }}</p>
<p>Status: {{ $payin->status }}</p>

@if ($payin->status == 'pending')
    <p>Your payment is still pending. Please wait for the confirmation.</p>
@else
    <p>Thank you! Your payment has been confirmed.</p>
@endif
</body>
</html>
