import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition, applyUrlDefaults } from './../../../../../wayfinder'
/**
* @see \App\Http\Controllers\Api\ChatController::index
* @see app/Http/Controllers/Api/ChatController.php:38
* @route '/api/chats'
*/
export const index = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: index.url(options),
    method: 'get',
})

index.definition = {
    methods: ["get","head"],
    url: '/api/chats',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\ChatController::index
* @see app/Http/Controllers/Api/ChatController.php:38
* @route '/api/chats'
*/
index.url = (options?: RouteQueryOptions) => {
    return index.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\ChatController::index
* @see app/Http/Controllers/Api/ChatController.php:38
* @route '/api/chats'
*/
index.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: index.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\ChatController::index
* @see app/Http/Controllers/Api/ChatController.php:38
* @route '/api/chats'
*/
index.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: index.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\ChatController::index
* @see app/Http/Controllers/Api/ChatController.php:38
* @route '/api/chats'
*/
const indexForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: index.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\ChatController::index
* @see app/Http/Controllers/Api/ChatController.php:38
* @route '/api/chats'
*/
indexForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: index.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\ChatController::index
* @see app/Http/Controllers/Api/ChatController.php:38
* @route '/api/chats'
*/
indexForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: index.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

index.form = indexForm

/**
* @see \App\Http\Controllers\Api\ChatController::store
* @see app/Http/Controllers/Api/ChatController.php:72
* @route '/api/chats'
*/
export const store = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: store.url(options),
    method: 'post',
})

store.definition = {
    methods: ["post"],
    url: '/api/chats',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\ChatController::store
* @see app/Http/Controllers/Api/ChatController.php:72
* @route '/api/chats'
*/
store.url = (options?: RouteQueryOptions) => {
    return store.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\ChatController::store
* @see app/Http/Controllers/Api/ChatController.php:72
* @route '/api/chats'
*/
store.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: store.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\ChatController::store
* @see app/Http/Controllers/Api/ChatController.php:72
* @route '/api/chats'
*/
const storeForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: store.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\ChatController::store
* @see app/Http/Controllers/Api/ChatController.php:72
* @route '/api/chats'
*/
storeForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: store.url(options),
    method: 'post',
})

store.form = storeForm

/**
* @see \App\Http\Controllers\Api\ChatController::show
* @see app/Http/Controllers/Api/ChatController.php:123
* @route '/api/chats/{conversationId}'
*/
export const show = (args: { conversationId: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: show.url(args, options),
    method: 'get',
})

show.definition = {
    methods: ["get","head"],
    url: '/api/chats/{conversationId}',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\ChatController::show
* @see app/Http/Controllers/Api/ChatController.php:123
* @route '/api/chats/{conversationId}'
*/
show.url = (args: { conversationId: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions) => {
    if (typeof args === 'string' || typeof args === 'number') {
        args = { conversationId: args }
    }

    if (Array.isArray(args)) {
        args = {
            conversationId: args[0],
        }
    }

    args = applyUrlDefaults(args)

    const parsedArgs = {
        conversationId: args.conversationId,
    }

    return show.definition.url
            .replace('{conversationId}', parsedArgs.conversationId.toString())
            .replace(/\/+$/, '') + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\ChatController::show
* @see app/Http/Controllers/Api/ChatController.php:123
* @route '/api/chats/{conversationId}'
*/
show.get = (args: { conversationId: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: show.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\ChatController::show
* @see app/Http/Controllers/Api/ChatController.php:123
* @route '/api/chats/{conversationId}'
*/
show.head = (args: { conversationId: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: show.url(args, options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\ChatController::show
* @see app/Http/Controllers/Api/ChatController.php:123
* @route '/api/chats/{conversationId}'
*/
const showForm = (args: { conversationId: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: show.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\ChatController::show
* @see app/Http/Controllers/Api/ChatController.php:123
* @route '/api/chats/{conversationId}'
*/
showForm.get = (args: { conversationId: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: show.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\ChatController::show
* @see app/Http/Controllers/Api/ChatController.php:123
* @route '/api/chats/{conversationId}'
*/
showForm.head = (args: { conversationId: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: show.url(args, {
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

show.form = showForm

/**
* @see \App\Http\Controllers\Api\ChatController::messages
* @see app/Http/Controllers/Api/ChatController.php:191
* @route '/api/chats/{conversationId}/messages'
*/
export const messages = (args: { conversationId: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: messages.url(args, options),
    method: 'get',
})

messages.definition = {
    methods: ["get","head"],
    url: '/api/chats/{conversationId}/messages',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\ChatController::messages
* @see app/Http/Controllers/Api/ChatController.php:191
* @route '/api/chats/{conversationId}/messages'
*/
messages.url = (args: { conversationId: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions) => {
    if (typeof args === 'string' || typeof args === 'number') {
        args = { conversationId: args }
    }

    if (Array.isArray(args)) {
        args = {
            conversationId: args[0],
        }
    }

    args = applyUrlDefaults(args)

    const parsedArgs = {
        conversationId: args.conversationId,
    }

    return messages.definition.url
            .replace('{conversationId}', parsedArgs.conversationId.toString())
            .replace(/\/+$/, '') + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\ChatController::messages
* @see app/Http/Controllers/Api/ChatController.php:191
* @route '/api/chats/{conversationId}/messages'
*/
messages.get = (args: { conversationId: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: messages.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\ChatController::messages
* @see app/Http/Controllers/Api/ChatController.php:191
* @route '/api/chats/{conversationId}/messages'
*/
messages.head = (args: { conversationId: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: messages.url(args, options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\ChatController::messages
* @see app/Http/Controllers/Api/ChatController.php:191
* @route '/api/chats/{conversationId}/messages'
*/
const messagesForm = (args: { conversationId: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: messages.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\ChatController::messages
* @see app/Http/Controllers/Api/ChatController.php:191
* @route '/api/chats/{conversationId}/messages'
*/
messagesForm.get = (args: { conversationId: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: messages.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\ChatController::messages
* @see app/Http/Controllers/Api/ChatController.php:191
* @route '/api/chats/{conversationId}/messages'
*/
messagesForm.head = (args: { conversationId: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: messages.url(args, {
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

messages.form = messagesForm

/**
* @see \App\Http\Controllers\Api\ChatController::runs
* @see app/Http/Controllers/Api/ChatController.php:222
* @route '/api/chats/{conversationId}/runs'
*/
export const runs = (args: { conversationId: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: runs.url(args, options),
    method: 'get',
})

runs.definition = {
    methods: ["get","head"],
    url: '/api/chats/{conversationId}/runs',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\ChatController::runs
* @see app/Http/Controllers/Api/ChatController.php:222
* @route '/api/chats/{conversationId}/runs'
*/
runs.url = (args: { conversationId: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions) => {
    if (typeof args === 'string' || typeof args === 'number') {
        args = { conversationId: args }
    }

    if (Array.isArray(args)) {
        args = {
            conversationId: args[0],
        }
    }

    args = applyUrlDefaults(args)

    const parsedArgs = {
        conversationId: args.conversationId,
    }

    return runs.definition.url
            .replace('{conversationId}', parsedArgs.conversationId.toString())
            .replace(/\/+$/, '') + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\ChatController::runs
* @see app/Http/Controllers/Api/ChatController.php:222
* @route '/api/chats/{conversationId}/runs'
*/
runs.get = (args: { conversationId: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: runs.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\ChatController::runs
* @see app/Http/Controllers/Api/ChatController.php:222
* @route '/api/chats/{conversationId}/runs'
*/
runs.head = (args: { conversationId: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: runs.url(args, options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\ChatController::runs
* @see app/Http/Controllers/Api/ChatController.php:222
* @route '/api/chats/{conversationId}/runs'
*/
const runsForm = (args: { conversationId: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: runs.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\ChatController::runs
* @see app/Http/Controllers/Api/ChatController.php:222
* @route '/api/chats/{conversationId}/runs'
*/
runsForm.get = (args: { conversationId: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: runs.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\ChatController::runs
* @see app/Http/Controllers/Api/ChatController.php:222
* @route '/api/chats/{conversationId}/runs'
*/
runsForm.head = (args: { conversationId: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: runs.url(args, {
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

runs.form = runsForm

/**
* @see \App\Http\Controllers\Api\ChatController::runEvents
* @see app/Http/Controllers/Api/ChatController.php:256
* @route '/api/chats/{conversationId}/runs/{runId}/events'
*/
export const runEvents = (args: { conversationId: string | number, runId: string | number } | [conversationId: string | number, runId: string | number ], options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: runEvents.url(args, options),
    method: 'get',
})

runEvents.definition = {
    methods: ["get","head"],
    url: '/api/chats/{conversationId}/runs/{runId}/events',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\ChatController::runEvents
* @see app/Http/Controllers/Api/ChatController.php:256
* @route '/api/chats/{conversationId}/runs/{runId}/events'
*/
runEvents.url = (args: { conversationId: string | number, runId: string | number } | [conversationId: string | number, runId: string | number ], options?: RouteQueryOptions) => {
    if (Array.isArray(args)) {
        args = {
            conversationId: args[0],
            runId: args[1],
        }
    }

    args = applyUrlDefaults(args)

    const parsedArgs = {
        conversationId: args.conversationId,
        runId: args.runId,
    }

    return runEvents.definition.url
            .replace('{conversationId}', parsedArgs.conversationId.toString())
            .replace('{runId}', parsedArgs.runId.toString())
            .replace(/\/+$/, '') + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\ChatController::runEvents
* @see app/Http/Controllers/Api/ChatController.php:256
* @route '/api/chats/{conversationId}/runs/{runId}/events'
*/
runEvents.get = (args: { conversationId: string | number, runId: string | number } | [conversationId: string | number, runId: string | number ], options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: runEvents.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\ChatController::runEvents
* @see app/Http/Controllers/Api/ChatController.php:256
* @route '/api/chats/{conversationId}/runs/{runId}/events'
*/
runEvents.head = (args: { conversationId: string | number, runId: string | number } | [conversationId: string | number, runId: string | number ], options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: runEvents.url(args, options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\ChatController::runEvents
* @see app/Http/Controllers/Api/ChatController.php:256
* @route '/api/chats/{conversationId}/runs/{runId}/events'
*/
const runEventsForm = (args: { conversationId: string | number, runId: string | number } | [conversationId: string | number, runId: string | number ], options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: runEvents.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\ChatController::runEvents
* @see app/Http/Controllers/Api/ChatController.php:256
* @route '/api/chats/{conversationId}/runs/{runId}/events'
*/
runEventsForm.get = (args: { conversationId: string | number, runId: string | number } | [conversationId: string | number, runId: string | number ], options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: runEvents.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\ChatController::runEvents
* @see app/Http/Controllers/Api/ChatController.php:256
* @route '/api/chats/{conversationId}/runs/{runId}/events'
*/
runEventsForm.head = (args: { conversationId: string | number, runId: string | number } | [conversationId: string | number, runId: string | number ], options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: runEvents.url(args, {
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

runEvents.form = runEventsForm

/**
* @see \App\Http\Controllers\Api\ChatController::stream
* @see app/Http/Controllers/Api/ChatController.php:330
* @route '/api/chats/{conversationId}/stream'
*/
const stream5947d79eb1c8fdae7df552fc22d245de = (args: { conversationId: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: stream5947d79eb1c8fdae7df552fc22d245de.url(args, options),
    method: 'post',
})

stream5947d79eb1c8fdae7df552fc22d245de.definition = {
    methods: ["post"],
    url: '/api/chats/{conversationId}/stream',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\ChatController::stream
* @see app/Http/Controllers/Api/ChatController.php:330
* @route '/api/chats/{conversationId}/stream'
*/
stream5947d79eb1c8fdae7df552fc22d245de.url = (args: { conversationId: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions) => {
    if (typeof args === 'string' || typeof args === 'number') {
        args = { conversationId: args }
    }

    if (Array.isArray(args)) {
        args = {
            conversationId: args[0],
        }
    }

    args = applyUrlDefaults(args)

    const parsedArgs = {
        conversationId: args.conversationId,
    }

    return stream5947d79eb1c8fdae7df552fc22d245de.definition.url
            .replace('{conversationId}', parsedArgs.conversationId.toString())
            .replace(/\/+$/, '') + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\ChatController::stream
* @see app/Http/Controllers/Api/ChatController.php:330
* @route '/api/chats/{conversationId}/stream'
*/
stream5947d79eb1c8fdae7df552fc22d245de.post = (args: { conversationId: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: stream5947d79eb1c8fdae7df552fc22d245de.url(args, options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\ChatController::stream
* @see app/Http/Controllers/Api/ChatController.php:330
* @route '/api/chats/{conversationId}/stream'
*/
const stream5947d79eb1c8fdae7df552fc22d245deForm = (args: { conversationId: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: stream5947d79eb1c8fdae7df552fc22d245de.url(args, options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\ChatController::stream
* @see app/Http/Controllers/Api/ChatController.php:330
* @route '/api/chats/{conversationId}/stream'
*/
stream5947d79eb1c8fdae7df552fc22d245deForm.post = (args: { conversationId: string | number } | [conversationId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: stream5947d79eb1c8fdae7df552fc22d245de.url(args, options),
    method: 'post',
})

stream5947d79eb1c8fdae7df552fc22d245de.form = stream5947d79eb1c8fdae7df552fc22d245deForm
/**
* @see \App\Http\Controllers\Api\ChatController::stream
* @see app/Http/Controllers/Api/ChatController.php:330
* @route '/api/chat/stream'
*/
const streamf4362dae03a9570caccf944945456f65 = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: streamf4362dae03a9570caccf944945456f65.url(options),
    method: 'post',
})

streamf4362dae03a9570caccf944945456f65.definition = {
    methods: ["post"],
    url: '/api/chat/stream',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\ChatController::stream
* @see app/Http/Controllers/Api/ChatController.php:330
* @route '/api/chat/stream'
*/
streamf4362dae03a9570caccf944945456f65.url = (options?: RouteQueryOptions) => {
    return streamf4362dae03a9570caccf944945456f65.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\ChatController::stream
* @see app/Http/Controllers/Api/ChatController.php:330
* @route '/api/chat/stream'
*/
streamf4362dae03a9570caccf944945456f65.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: streamf4362dae03a9570caccf944945456f65.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\ChatController::stream
* @see app/Http/Controllers/Api/ChatController.php:330
* @route '/api/chat/stream'
*/
const streamf4362dae03a9570caccf944945456f65Form = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: streamf4362dae03a9570caccf944945456f65.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\ChatController::stream
* @see app/Http/Controllers/Api/ChatController.php:330
* @route '/api/chat/stream'
*/
streamf4362dae03a9570caccf944945456f65Form.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: streamf4362dae03a9570caccf944945456f65.url(options),
    method: 'post',
})

streamf4362dae03a9570caccf944945456f65.form = streamf4362dae03a9570caccf944945456f65Form

export const stream = {
    '/api/chats/{conversationId}/stream': stream5947d79eb1c8fdae7df552fc22d245de,
    '/api/chat/stream': streamf4362dae03a9570caccf944945456f65,
}

const ChatController = { index, store, show, messages, runs, runEvents, stream }

export default ChatController