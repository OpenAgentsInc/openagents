<?php

return [
    'npub_resolver' => env('NOSTR_NPUB_RESOLVER') ?: 'https://granary.io/nostr/{{$npub}}/%40all/@app/?format=nostr&relay=relay.nos.social',
    'npub_opener' => env('NOSTR_NPUB_OPENER') ?: 'https://nostr.com/{{$npub}}',
];
