import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition } from './../../../wayfinder'
/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::email
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:51
* @route '/api/auth/email'
*/
export const email = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: email.url(options),
    method: 'post',
})

email.definition = {
    methods: ["post"],
    url: '/api/auth/email',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::email
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:51
* @route '/api/auth/email'
*/
email.url = (options?: RouteQueryOptions) => {
    return email.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::email
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:51
* @route '/api/auth/email'
*/
email.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: email.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::email
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:51
* @route '/api/auth/email'
*/
const emailForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: email.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::email
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:51
* @route '/api/auth/email'
*/
emailForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: email.url(options),
    method: 'post',
})

email.form = emailForm

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::verify
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:85
* @route '/api/auth/verify'
*/
export const verify = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: verify.url(options),
    method: 'post',
})

verify.definition = {
    methods: ["post"],
    url: '/api/auth/verify',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::verify
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:85
* @route '/api/auth/verify'
*/
verify.url = (options?: RouteQueryOptions) => {
    return verify.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::verify
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:85
* @route '/api/auth/verify'
*/
verify.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: verify.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::verify
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:85
* @route '/api/auth/verify'
*/
const verifyForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: verify.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Auth\EmailCodeAuthController::verify
* @see app/Http/Controllers/Auth/EmailCodeAuthController.php:85
* @route '/api/auth/verify'
*/
verifyForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: verify.url(options),
    method: 'post',
})

verify.form = verifyForm

const auth = {
    email: Object.assign(email, email),
    verify: Object.assign(verify, verify),
}

export default auth