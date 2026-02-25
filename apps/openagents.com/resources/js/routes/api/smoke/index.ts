import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition } from './../../../wayfinder'
/**
* @see routes/web.php:33
* @route '/api/smoke/stream'
*/
export const stream = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: stream.url(options),
    method: 'get',
})

stream.definition = {
    methods: ["get","head"],
    url: '/api/smoke/stream',
} satisfies RouteDefinition<["get","head"]>

/**
* @see routes/web.php:33
* @route '/api/smoke/stream'
*/
stream.url = (options?: RouteQueryOptions) => {
    return stream.definition.url + queryParams(options)
}

/**
* @see routes/web.php:33
* @route '/api/smoke/stream'
*/
stream.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: stream.url(options),
    method: 'get',
})

/**
* @see routes/web.php:33
* @route '/api/smoke/stream'
*/
stream.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: stream.url(options),
    method: 'head',
})

/**
* @see routes/web.php:33
* @route '/api/smoke/stream'
*/
const streamForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: stream.url(options),
    method: 'get',
})

/**
* @see routes/web.php:33
* @route '/api/smoke/stream'
*/
streamForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: stream.url(options),
    method: 'get',
})

/**
* @see routes/web.php:33
* @route '/api/smoke/stream'
*/
streamForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: stream.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

stream.form = streamForm

const smoke = {
    stream: Object.assign(stream, stream),
}

export default smoke