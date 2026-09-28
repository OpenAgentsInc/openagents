package com.openagents.app

import kotlin.math.hypot

/** Native contact arbitration; camera and movement state remain in Rust. */
internal class PinchAdmission {
    private data class Contact(val began: Long, val x: Float, val y: Float, var moved: Boolean = false)
    private val contacts = mutableMapOf<Int, Contact>()
    var reserved = false; private set
    val allowed get() = reserved && contacts.size == 2
    fun down(id: Int, x: Float, y: Float, time: Long) {
        if (contacts.isEmpty()) reserved = false
        val first = contacts.values.singleOrNull()
        if (first != null && time - first.began in 0..150 && !first.moved) reserved = true
        contacts[id] = Contact(time, x, y)
    }
    fun move(id: Int, x: Float, y: Float) {
        contacts[id]?.let { it.moved = it.moved || hypot(x - it.x, y - it.y) > 8f }
    }
    fun up(id: Int) { contacts.remove(id); if (contacts.isEmpty()) reset() }
    fun reset() { contacts.clear(); reserved = false }
}
