import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition, applyUrlDefaults } from './../../../../../wayfinder'
/**
* @see \App\Http\Controllers\Api\AutopilotController::index
* @see app/Http/Controllers/Api/AutopilotController.php:35
* @route '/api/autopilots'
*/
export const index = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: index.url(options),
    method: 'get',
})

index.definition = {
    methods: ["get","head"],
    url: '/api/autopilots',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\AutopilotController::index
* @see app/Http/Controllers/Api/AutopilotController.php:35
* @route '/api/autopilots'
*/
index.url = (options?: RouteQueryOptions) => {
    return index.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\AutopilotController::index
* @see app/Http/Controllers/Api/AutopilotController.php:35
* @route '/api/autopilots'
*/
index.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: index.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\AutopilotController::index
* @see app/Http/Controllers/Api/AutopilotController.php:35
* @route '/api/autopilots'
*/
index.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: index.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\AutopilotController::index
* @see app/Http/Controllers/Api/AutopilotController.php:35
* @route '/api/autopilots'
*/
const indexForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: index.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\AutopilotController::index
* @see app/Http/Controllers/Api/AutopilotController.php:35
* @route '/api/autopilots'
*/
indexForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: index.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\AutopilotController::index
* @see app/Http/Controllers/Api/AutopilotController.php:35
* @route '/api/autopilots'
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
* @see \App\Http\Controllers\Api\AutopilotController::store
* @see app/Http/Controllers/Api/AutopilotController.php:68
* @route '/api/autopilots'
*/
export const store = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: store.url(options),
    method: 'post',
})

store.definition = {
    methods: ["post"],
    url: '/api/autopilots',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\AutopilotController::store
* @see app/Http/Controllers/Api/AutopilotController.php:68
* @route '/api/autopilots'
*/
store.url = (options?: RouteQueryOptions) => {
    return store.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\AutopilotController::store
* @see app/Http/Controllers/Api/AutopilotController.php:68
* @route '/api/autopilots'
*/
store.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: store.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\AutopilotController::store
* @see app/Http/Controllers/Api/AutopilotController.php:68
* @route '/api/autopilots'
*/
const storeForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: store.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\AutopilotController::store
* @see app/Http/Controllers/Api/AutopilotController.php:68
* @route '/api/autopilots'
*/
storeForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: store.url(options),
    method: 'post',
})

store.form = storeForm

/**
* @see \App\Http\Controllers\Api\AutopilotController::show
* @see app/Http/Controllers/Api/AutopilotController.php:104
* @route '/api/autopilots/{autopilot}'
*/
export const show = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: show.url(args, options),
    method: 'get',
})

show.definition = {
    methods: ["get","head"],
    url: '/api/autopilots/{autopilot}',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\AutopilotController::show
* @see app/Http/Controllers/Api/AutopilotController.php:104
* @route '/api/autopilots/{autopilot}'
*/
show.url = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions) => {
    if (typeof args === 'string' || typeof args === 'number') {
        args = { autopilot: args }
    }

    if (Array.isArray(args)) {
        args = {
            autopilot: args[0],
        }
    }

    args = applyUrlDefaults(args)

    const parsedArgs = {
        autopilot: args.autopilot,
    }

    return show.definition.url
            .replace('{autopilot}', parsedArgs.autopilot.toString())
            .replace(/\/+$/, '') + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\AutopilotController::show
* @see app/Http/Controllers/Api/AutopilotController.php:104
* @route '/api/autopilots/{autopilot}'
*/
show.get = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: show.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\AutopilotController::show
* @see app/Http/Controllers/Api/AutopilotController.php:104
* @route '/api/autopilots/{autopilot}'
*/
show.head = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: show.url(args, options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\AutopilotController::show
* @see app/Http/Controllers/Api/AutopilotController.php:104
* @route '/api/autopilots/{autopilot}'
*/
const showForm = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: show.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\AutopilotController::show
* @see app/Http/Controllers/Api/AutopilotController.php:104
* @route '/api/autopilots/{autopilot}'
*/
showForm.get = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: show.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\AutopilotController::show
* @see app/Http/Controllers/Api/AutopilotController.php:104
* @route '/api/autopilots/{autopilot}'
*/
showForm.head = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
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
* @see \App\Http\Controllers\Api\AutopilotController::update
* @see app/Http/Controllers/Api/AutopilotController.php:131
* @route '/api/autopilots/{autopilot}'
*/
export const update = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'patch'> => ({
    url: update.url(args, options),
    method: 'patch',
})

update.definition = {
    methods: ["patch"],
    url: '/api/autopilots/{autopilot}',
} satisfies RouteDefinition<["patch"]>

/**
* @see \App\Http\Controllers\Api\AutopilotController::update
* @see app/Http/Controllers/Api/AutopilotController.php:131
* @route '/api/autopilots/{autopilot}'
*/
update.url = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions) => {
    if (typeof args === 'string' || typeof args === 'number') {
        args = { autopilot: args }
    }

    if (Array.isArray(args)) {
        args = {
            autopilot: args[0],
        }
    }

    args = applyUrlDefaults(args)

    const parsedArgs = {
        autopilot: args.autopilot,
    }

    return update.definition.url
            .replace('{autopilot}', parsedArgs.autopilot.toString())
            .replace(/\/+$/, '') + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\AutopilotController::update
* @see app/Http/Controllers/Api/AutopilotController.php:131
* @route '/api/autopilots/{autopilot}'
*/
update.patch = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'patch'> => ({
    url: update.url(args, options),
    method: 'patch',
})

/**
* @see \App\Http\Controllers\Api\AutopilotController::update
* @see app/Http/Controllers/Api/AutopilotController.php:131
* @route '/api/autopilots/{autopilot}'
*/
const updateForm = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: update.url(args, {
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'PATCH',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\AutopilotController::update
* @see app/Http/Controllers/Api/AutopilotController.php:131
* @route '/api/autopilots/{autopilot}'
*/
updateForm.patch = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: update.url(args, {
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'PATCH',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'post',
})

update.form = updateForm

/**
* @see \App\Http\Controllers\Api\AutopilotController::threads
* @see app/Http/Controllers/Api/AutopilotController.php:229
* @route '/api/autopilots/{autopilot}/threads'
*/
export const threads = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: threads.url(args, options),
    method: 'get',
})

threads.definition = {
    methods: ["get","head"],
    url: '/api/autopilots/{autopilot}/threads',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\AutopilotController::threads
* @see app/Http/Controllers/Api/AutopilotController.php:229
* @route '/api/autopilots/{autopilot}/threads'
*/
threads.url = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions) => {
    if (typeof args === 'string' || typeof args === 'number') {
        args = { autopilot: args }
    }

    if (Array.isArray(args)) {
        args = {
            autopilot: args[0],
        }
    }

    args = applyUrlDefaults(args)

    const parsedArgs = {
        autopilot: args.autopilot,
    }

    return threads.definition.url
            .replace('{autopilot}', parsedArgs.autopilot.toString())
            .replace(/\/+$/, '') + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\AutopilotController::threads
* @see app/Http/Controllers/Api/AutopilotController.php:229
* @route '/api/autopilots/{autopilot}/threads'
*/
threads.get = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: threads.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\AutopilotController::threads
* @see app/Http/Controllers/Api/AutopilotController.php:229
* @route '/api/autopilots/{autopilot}/threads'
*/
threads.head = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: threads.url(args, options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\AutopilotController::threads
* @see app/Http/Controllers/Api/AutopilotController.php:229
* @route '/api/autopilots/{autopilot}/threads'
*/
const threadsForm = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: threads.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\AutopilotController::threads
* @see app/Http/Controllers/Api/AutopilotController.php:229
* @route '/api/autopilots/{autopilot}/threads'
*/
threadsForm.get = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: threads.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\AutopilotController::threads
* @see app/Http/Controllers/Api/AutopilotController.php:229
* @route '/api/autopilots/{autopilot}/threads'
*/
threadsForm.head = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: threads.url(args, {
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

threads.form = threadsForm

/**
* @see \App\Http\Controllers\Api\AutopilotController::storeThread
* @see app/Http/Controllers/Api/AutopilotController.php:191
* @route '/api/autopilots/{autopilot}/threads'
*/
export const storeThread = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: storeThread.url(args, options),
    method: 'post',
})

storeThread.definition = {
    methods: ["post"],
    url: '/api/autopilots/{autopilot}/threads',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\AutopilotController::storeThread
* @see app/Http/Controllers/Api/AutopilotController.php:191
* @route '/api/autopilots/{autopilot}/threads'
*/
storeThread.url = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions) => {
    if (typeof args === 'string' || typeof args === 'number') {
        args = { autopilot: args }
    }

    if (Array.isArray(args)) {
        args = {
            autopilot: args[0],
        }
    }

    args = applyUrlDefaults(args)

    const parsedArgs = {
        autopilot: args.autopilot,
    }

    return storeThread.definition.url
            .replace('{autopilot}', parsedArgs.autopilot.toString())
            .replace(/\/+$/, '') + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\AutopilotController::storeThread
* @see app/Http/Controllers/Api/AutopilotController.php:191
* @route '/api/autopilots/{autopilot}/threads'
*/
storeThread.post = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: storeThread.url(args, options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\AutopilotController::storeThread
* @see app/Http/Controllers/Api/AutopilotController.php:191
* @route '/api/autopilots/{autopilot}/threads'
*/
const storeThreadForm = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: storeThread.url(args, options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\AutopilotController::storeThread
* @see app/Http/Controllers/Api/AutopilotController.php:191
* @route '/api/autopilots/{autopilot}/threads'
*/
storeThreadForm.post = (args: { autopilot: string | number } | [autopilot: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: storeThread.url(args, options),
    method: 'post',
})

storeThread.form = storeThreadForm

const AutopilotController = { index, store, show, update, threads, storeThread }

export default AutopilotController