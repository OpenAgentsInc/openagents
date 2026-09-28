package com.openagents.app

import kotlin.math.hypot

/**
 * Native contact arbitration; camera and movement state remain in Rust.
 * Two fresh, unmoved contacts that land within 150 ms reserve a pinch, and
 * the pinch's scale comes from those two contacts alone. The host keeps the
 * pointers Rust took for the movement and look sticks out of this class, so
 * a pinch with other fingers never claims or moves them.
 */
internal class PinchAdmission {
    private class Contact(val began: Long, val x: Float, val y: Float) {
        var currentX = x
        var currentY = y
        var moved = false
    }
    private val contacts = linkedMapOf<Int, Contact>()
    private var previousDistance: Float? = null
    var reserved = false; private set
    val allowed get() = reserved && contacts.size == 2

    fun down(id: Int, x: Float, y: Float, time: Long) {
        if (contacts.isEmpty()) reset()
        val first = contacts.values.singleOrNull()
        if (first != null && time - first.began in 0..150 && !first.moved) reserved = true
        contacts[id] = Contact(time, x, y)
    }

    fun move(id: Int, x: Float, y: Float) {
        val contact = contacts[id] ?: return
        contact.moved = contact.moved || hypot(x - contact.x, y - contact.y) > 8f
        contact.currentX = x; contact.currentY = y
    }

    /**
     * The pinch's scale since the last sample, after each event batch. The
     * first sample of a pinch anchors it and returns null, so an old scale
     * never applies.
     */
    fun scale(): Float? {
        if (!allowed) { previousDistance = null; return null }
        val (a, b) = contacts.values.toList()
        val distance = hypot(a.currentX - b.currentX, a.currentY - b.currentY)
        if (!distance.isFinite() || distance < 1f) { previousDistance = null; return null }
        val previous = previousDistance
        previousDistance = distance
        return previous?.let { distance / it }
    }

    fun contains(id: Int) = id in contacts

    fun up(id: Int) {
        contacts.remove(id)
        previousDistance = null
        if (contacts.isEmpty()) reset()
    }

    fun reset() { contacts.clear(); reserved = false; previousDistance = null }
}
