import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition, applyUrlDefaults } from './../../../../../wayfinder'
/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::index
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:13
* @route '/api/runtime/codex/workers'
*/
export const index = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: index.url(options),
    method: 'get',
})

index.definition = {
    methods: ["get","head"],
    url: '/api/runtime/codex/workers',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::index
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:13
* @route '/api/runtime/codex/workers'
*/
index.url = (options?: RouteQueryOptions) => {
    return index.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::index
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:13
* @route '/api/runtime/codex/workers'
*/
index.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: index.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::index
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:13
* @route '/api/runtime/codex/workers'
*/
index.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: index.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::index
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:13
* @route '/api/runtime/codex/workers'
*/
const indexForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: index.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::index
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:13
* @route '/api/runtime/codex/workers'
*/
indexForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: index.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::index
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:13
* @route '/api/runtime/codex/workers'
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
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::create
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:52
* @route '/api/runtime/codex/workers'
*/
export const create = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: create.url(options),
    method: 'post',
})

create.definition = {
    methods: ["post"],
    url: '/api/runtime/codex/workers',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::create
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:52
* @route '/api/runtime/codex/workers'
*/
create.url = (options?: RouteQueryOptions) => {
    return create.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::create
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:52
* @route '/api/runtime/codex/workers'
*/
create.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: create.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::create
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:52
* @route '/api/runtime/codex/workers'
*/
const createForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: create.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::create
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:52
* @route '/api/runtime/codex/workers'
*/
createForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: create.url(options),
    method: 'post',
})

create.form = createForm

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::show
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:76
* @route '/api/runtime/codex/workers/{workerId}'
*/
export const show = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: show.url(args, options),
    method: 'get',
})

show.definition = {
    methods: ["get","head"],
    url: '/api/runtime/codex/workers/{workerId}',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::show
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:76
* @route '/api/runtime/codex/workers/{workerId}'
*/
show.url = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions) => {
    if (typeof args === 'string' || typeof args === 'number') {
        args = { workerId: args }
    }

    if (Array.isArray(args)) {
        args = {
            workerId: args[0],
        }
    }

    args = applyUrlDefaults(args)

    const parsedArgs = {
        workerId: args.workerId,
    }

    return show.definition.url
            .replace('{workerId}', parsedArgs.workerId.toString())
            .replace(/\/+$/, '') + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::show
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:76
* @route '/api/runtime/codex/workers/{workerId}'
*/
show.get = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: show.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::show
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:76
* @route '/api/runtime/codex/workers/{workerId}'
*/
show.head = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: show.url(args, options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::show
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:76
* @route '/api/runtime/codex/workers/{workerId}'
*/
const showForm = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: show.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::show
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:76
* @route '/api/runtime/codex/workers/{workerId}'
*/
showForm.get = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: show.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::show
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:76
* @route '/api/runtime/codex/workers/{workerId}'
*/
showForm.head = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
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
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::stream
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:158
* @route '/api/runtime/codex/workers/{workerId}/stream'
*/
export const stream = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: stream.url(args, options),
    method: 'get',
})

stream.definition = {
    methods: ["get","head"],
    url: '/api/runtime/codex/workers/{workerId}/stream',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::stream
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:158
* @route '/api/runtime/codex/workers/{workerId}/stream'
*/
stream.url = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions) => {
    if (typeof args === 'string' || typeof args === 'number') {
        args = { workerId: args }
    }

    if (Array.isArray(args)) {
        args = {
            workerId: args[0],
        }
    }

    args = applyUrlDefaults(args)

    const parsedArgs = {
        workerId: args.workerId,
    }

    return stream.definition.url
            .replace('{workerId}', parsedArgs.workerId.toString())
            .replace(/\/+$/, '') + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::stream
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:158
* @route '/api/runtime/codex/workers/{workerId}/stream'
*/
stream.get = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: stream.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::stream
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:158
* @route '/api/runtime/codex/workers/{workerId}/stream'
*/
stream.head = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: stream.url(args, options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::stream
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:158
* @route '/api/runtime/codex/workers/{workerId}/stream'
*/
const streamForm = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: stream.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::stream
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:158
* @route '/api/runtime/codex/workers/{workerId}/stream'
*/
streamForm.get = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: stream.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::stream
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:158
* @route '/api/runtime/codex/workers/{workerId}/stream'
*/
streamForm.head = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: stream.url(args, {
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

stream.form = streamForm

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::request
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:93
* @route '/api/runtime/codex/workers/{workerId}/requests'
*/
export const request = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: request.url(args, options),
    method: 'post',
})

request.definition = {
    methods: ["post"],
    url: '/api/runtime/codex/workers/{workerId}/requests',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::request
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:93
* @route '/api/runtime/codex/workers/{workerId}/requests'
*/
request.url = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions) => {
    if (typeof args === 'string' || typeof args === 'number') {
        args = { workerId: args }
    }

    if (Array.isArray(args)) {
        args = {
            workerId: args[0],
        }
    }

    args = applyUrlDefaults(args)

    const parsedArgs = {
        workerId: args.workerId,
    }

    return request.definition.url
            .replace('{workerId}', parsedArgs.workerId.toString())
            .replace(/\/+$/, '') + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::request
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:93
* @route '/api/runtime/codex/workers/{workerId}/requests'
*/
request.post = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: request.url(args, options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::request
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:93
* @route '/api/runtime/codex/workers/{workerId}/requests'
*/
const requestForm = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: request.url(args, options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::request
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:93
* @route '/api/runtime/codex/workers/{workerId}/requests'
*/
requestForm.post = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: request.url(args, options),
    method: 'post',
})

request.form = requestForm

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::events
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:114
* @route '/api/runtime/codex/workers/{workerId}/events'
*/
export const events = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: events.url(args, options),
    method: 'post',
})

events.definition = {
    methods: ["post"],
    url: '/api/runtime/codex/workers/{workerId}/events',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::events
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:114
* @route '/api/runtime/codex/workers/{workerId}/events'
*/
events.url = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions) => {
    if (typeof args === 'string' || typeof args === 'number') {
        args = { workerId: args }
    }

    if (Array.isArray(args)) {
        args = {
            workerId: args[0],
        }
    }

    args = applyUrlDefaults(args)

    const parsedArgs = {
        workerId: args.workerId,
    }

    return events.definition.url
            .replace('{workerId}', parsedArgs.workerId.toString())
            .replace(/\/+$/, '') + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::events
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:114
* @route '/api/runtime/codex/workers/{workerId}/events'
*/
events.post = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: events.url(args, options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::events
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:114
* @route '/api/runtime/codex/workers/{workerId}/events'
*/
const eventsForm = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: events.url(args, options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::events
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:114
* @route '/api/runtime/codex/workers/{workerId}/events'
*/
eventsForm.post = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: events.url(args, options),
    method: 'post',
})

events.form = eventsForm

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::stop
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:137
* @route '/api/runtime/codex/workers/{workerId}/stop'
*/
export const stop = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: stop.url(args, options),
    method: 'post',
})

stop.definition = {
    methods: ["post"],
    url: '/api/runtime/codex/workers/{workerId}/stop',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::stop
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:137
* @route '/api/runtime/codex/workers/{workerId}/stop'
*/
stop.url = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions) => {
    if (typeof args === 'string' || typeof args === 'number') {
        args = { workerId: args }
    }

    if (Array.isArray(args)) {
        args = {
            workerId: args[0],
        }
    }

    args = applyUrlDefaults(args)

    const parsedArgs = {
        workerId: args.workerId,
    }

    return stop.definition.url
            .replace('{workerId}', parsedArgs.workerId.toString())
            .replace(/\/+$/, '') + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::stop
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:137
* @route '/api/runtime/codex/workers/{workerId}/stop'
*/
stop.post = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: stop.url(args, options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::stop
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:137
* @route '/api/runtime/codex/workers/{workerId}/stop'
*/
const stopForm = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: stop.url(args, options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\RuntimeCodexWorkersController::stop
* @see app/Http/Controllers/Api/RuntimeCodexWorkersController.php:137
* @route '/api/runtime/codex/workers/{workerId}/stop'
*/
stopForm.post = (args: { workerId: string | number } | [workerId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: stop.url(args, options),
    method: 'post',
})

stop.form = stopForm

const RuntimeCodexWorkersController = { index, create, show, stream, request, events, stop }

export default RuntimeCodexWorkersController