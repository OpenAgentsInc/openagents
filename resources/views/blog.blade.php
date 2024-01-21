@extends('layouts.main')

@section('title', 'OpenAgents Blog')

@section('content')

<div class="prose dark:prose-invert mx-auto">
    {!! $htmlContent !!}
</div>

<div class="mt-12 mx-auto w-[560px] max-w-full">
    <blockquote class="twitter-tweet" data-media-max-width="560">
        <p lang="en" dir="ltr">The Convergence of AI &amp; Bitcoin by Christopher David <a
                href="https://t.co/5dN97Iothw">https://t.co/5dN97Iothw</a></p>&mdash; PlebLab (@PlebLab) <a
            href="https://twitter.com/PlebLab/status/1748784406482026810?ref_src=twsrc%5Etfw">January 20, 2024</a>
    </blockquote>
    <script async src="https://platform.twitter.com/widgets.js" charset="utf-8"></script>
</div>

@endsection
