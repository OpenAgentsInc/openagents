import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition, applyUrlDefaults } from './../../../../../wayfinder'
/**
* @see \App\Http\Controllers\Api\WhispersController::index
* @see app/Http/Controllers/Api/WhispersController.php:41
* @route '/api/whispers'
*/
export const index = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: index.url(options),
    method: 'get',
})

index.definition = {
    methods: ["get","head"],
    url: '/api/whispers',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\WhispersController::index
* @see app/Http/Controllers/Api/WhispersController.php:41
* @route '/api/whispers'
*/
index.url = (options?: RouteQueryOptions) => {
    return index.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\WhispersController::index
* @see app/Http/Controllers/Api/WhispersController.php:41
* @route '/api/whispers'
*/
index.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: index.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\WhispersController::index
* @see app/Http/Controllers/Api/WhispersController.php:41
* @route '/api/whispers'
*/
index.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: index.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\WhispersController::index
* @see app/Http/Controllers/Api/WhispersController.php:41
* @route '/api/whispers'
*/
const indexForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: index.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\WhispersController::index
* @see app/Http/Controllers/Api/WhispersController.php:41
* @route '/api/whispers'
*/
indexForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: index.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\WhispersController::index
* @see app/Http/Controllers/Api/WhispersController.php:41
* @route '/api/whispers'
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
* @see \App\Http\Controllers\Api\WhispersController::store
* @see app/Http/Controllers/Api/WhispersController.php:96
* @route '/api/whispers'
*/
export const store = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: store.url(options),
    method: 'post',
})

store.definition = {
    methods: ["post"],
    url: '/api/whispers',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\WhispersController::store
* @see app/Http/Controllers/Api/WhispersController.php:96
* @route '/api/whispers'
*/
store.url = (options?: RouteQueryOptions) => {
    return store.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\WhispersController::store
* @see app/Http/Controllers/Api/WhispersController.php:96
* @route '/api/whispers'
*/
store.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: store.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\WhispersController::store
* @see app/Http/Controllers/Api/WhispersController.php:96
* @route '/api/whispers'
*/
const storeForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: store.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\WhispersController::store
* @see app/Http/Controllers/Api/WhispersController.php:96
* @route '/api/whispers'
*/
storeForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: store.url(options),
    method: 'post',
})

store.form = storeForm

/**
* @see \App\Http\Controllers\Api\WhispersController::read
* @see app/Http/Controllers/Api/WhispersController.php:145
* @route '/api/whispers/{id}/read'
*/
export const read = (args: { id: string | number } | [id: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'patch'> => ({
    url: read.url(args, options),
    method: 'patch',
})

read.definition = {
    methods: ["patch"],
    url: '/api/whispers/{id}/read',
} satisfies RouteDefinition<["patch"]>

/**
* @see \App\Http\Controllers\Api\WhispersController::read
* @see app/Http/Controllers/Api/WhispersController.php:145
* @route '/api/whispers/{id}/read'
*/
read.url = (args: { id: string | number } | [id: string | number ] | string | number, options?: RouteQueryOptions) => {
    if (typeof args === 'string' || typeof args === 'number') {
        args = { id: args }
    }

    if (Array.isArray(args)) {
        args = {
            id: args[0],
        }
    }

    args = applyUrlDefaults(args)

    const parsedArgs = {
        id: args.id,
    }

    return read.definition.url
            .replace('{id}', parsedArgs.id.toString())
            .replace(/\/+$/, '') + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\WhispersController::read
* @see app/Http/Controllers/Api/WhispersController.php:145
* @route '/api/whispers/{id}/read'
*/
read.patch = (args: { id: string | number } | [id: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'patch'> => ({
    url: read.url(args, options),
    method: 'patch',
})

/**
* @see \App\Http\Controllers\Api\WhispersController::read
* @see app/Http/Controllers/Api/WhispersController.php:145
* @route '/api/whispers/{id}/read'
*/
const readForm = (args: { id: string | number } | [id: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: read.url(args, {
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'PATCH',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\WhispersController::read
* @see app/Http/Controllers/Api/WhispersController.php:145
* @route '/api/whispers/{id}/read'
*/
readForm.patch = (args: { id: string | number } | [id: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: read.url(args, {
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'PATCH',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'post',
})

read.form = readForm

const WhispersController = { index, store, read }

export default WhispersController