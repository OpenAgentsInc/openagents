import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition, applyUrlDefaults } from './../../../wayfinder'
/**
* @see \App\Http\Controllers\L402PageController::index
* @see app/Http/Controllers/L402PageController.php:54
* @route '/l402/transactions'
*/
export const index = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: index.url(options),
    method: 'get',
})

index.definition = {
    methods: ["get","head"],
    url: '/l402/transactions',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\L402PageController::index
* @see app/Http/Controllers/L402PageController.php:54
* @route '/l402/transactions'
*/
index.url = (options?: RouteQueryOptions) => {
    return index.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\L402PageController::index
* @see app/Http/Controllers/L402PageController.php:54
* @route '/l402/transactions'
*/
index.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: index.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\L402PageController::index
* @see app/Http/Controllers/L402PageController.php:54
* @route '/l402/transactions'
*/
index.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: index.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\L402PageController::index
* @see app/Http/Controllers/L402PageController.php:54
* @route '/l402/transactions'
*/
const indexForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: index.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\L402PageController::index
* @see app/Http/Controllers/L402PageController.php:54
* @route '/l402/transactions'
*/
indexForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: index.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\L402PageController::index
* @see app/Http/Controllers/L402PageController.php:54
* @route '/l402/transactions'
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
* @see \App\Http\Controllers\L402PageController::show
* @see app/Http/Controllers/L402PageController.php:77
* @route '/l402/transactions/{eventId}'
*/
export const show = (args: { eventId: string | number } | [eventId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: show.url(args, options),
    method: 'get',
})

show.definition = {
    methods: ["get","head"],
    url: '/l402/transactions/{eventId}',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\L402PageController::show
* @see app/Http/Controllers/L402PageController.php:77
* @route '/l402/transactions/{eventId}'
*/
show.url = (args: { eventId: string | number } | [eventId: string | number ] | string | number, options?: RouteQueryOptions) => {
    if (typeof args === 'string' || typeof args === 'number') {
        args = { eventId: args }
    }

    if (Array.isArray(args)) {
        args = {
            eventId: args[0],
        }
    }

    args = applyUrlDefaults(args)

    const parsedArgs = {
        eventId: args.eventId,
    }

    return show.definition.url
            .replace('{eventId}', parsedArgs.eventId.toString())
            .replace(/\/+$/, '') + queryParams(options)
}

/**
* @see \App\Http\Controllers\L402PageController::show
* @see app/Http/Controllers/L402PageController.php:77
* @route '/l402/transactions/{eventId}'
*/
show.get = (args: { eventId: string | number } | [eventId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: show.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\L402PageController::show
* @see app/Http/Controllers/L402PageController.php:77
* @route '/l402/transactions/{eventId}'
*/
show.head = (args: { eventId: string | number } | [eventId: string | number ] | string | number, options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: show.url(args, options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\L402PageController::show
* @see app/Http/Controllers/L402PageController.php:77
* @route '/l402/transactions/{eventId}'
*/
const showForm = (args: { eventId: string | number } | [eventId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: show.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\L402PageController::show
* @see app/Http/Controllers/L402PageController.php:77
* @route '/l402/transactions/{eventId}'
*/
showForm.get = (args: { eventId: string | number } | [eventId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: show.url(args, options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\L402PageController::show
* @see app/Http/Controllers/L402PageController.php:77
* @route '/l402/transactions/{eventId}'
*/
showForm.head = (args: { eventId: string | number } | [eventId: string | number ] | string | number, options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: show.url(args, {
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

show.form = showForm

const transactions = {
    index: Object.assign(index, index),
    show: Object.assign(show, show),
}

export default transactions