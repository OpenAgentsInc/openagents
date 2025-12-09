/**
 * Widget Types
 *
 * Widgets are a simplified form of Components with a slightly different API.
 * For now, Widget is an alias for Component.
 */

import type { Component } from "../component/types.js"

/**
 * Widget type - alias for Component
 */
export type Widget<S, E, R = never> = Component<S, E, R>
