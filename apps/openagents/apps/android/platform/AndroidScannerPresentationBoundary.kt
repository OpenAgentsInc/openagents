package com.openagents.app

/** Scanner presentation states. Permission and camera implementations stay in the app shell. */
enum class AndroidScannerState {
    READY,
    PERMISSION_PENDING,
    SCANNING,
    PERMISSION_DENIED,
    CANCELLED,
    CAMERA_ERROR,
    SUBMITTED,
    CLOSED,
}

/** Result of offering a camera frame's text to the Rust app-link classifier. */
enum class AndroidScanResult {
    SUBMITTED_TO_RUST,
    DUPLICATE,
    STALE_SESSION,
    NOT_SCANNING,
    EMPTY,
    TOO_LARGE,
}

/**
 * Fences camera callbacks and forwards one bounded, unmodified scan result to Rust.
 *
 * The app shell must pass a callback that invokes the generated Rust app-link classifier.
 * This class does not parse links, authorize actions, or perform camera operations.
 */
class AndroidScannerPresentationBoundary(
    private val classifyWithRust: (String) -> Unit,
    private val maxUtf8Bytes: Int = MAX_SCAN_BYTES,
) {
    private val lock = Any()
    private var nextSession = 0L
    private var activeSession: Long? = null
    private var currentState = AndroidScannerState.READY

    init {
        require(maxUtf8Bytes in 1..MAX_SCAN_BYTES)
    }

    val state: AndroidScannerState
        get() = synchronized(lock) { currentState }

    /** Begin a permission request after the person opens the scanner. */
    fun requestPermission(): Long? = synchronized(lock) {
        if (currentState != AndroidScannerState.READY) return@synchronized null
        beginPermissionRequest()
    }

    /** Retry after an explicit denial or cancellation. The shell requests permission again. */
    fun retry(): Long? = synchronized(lock) {
        if (currentState !in setOf(
                AndroidScannerState.PERMISSION_DENIED,
                AndroidScannerState.CANCELLED,
                AndroidScannerState.CAMERA_ERROR,
            )
        ) {
            return@synchronized null
        }
        beginPermissionRequest()
    }

    /** Accept only the result for the current permission request. */
    fun permissionGranted(session: Long): Boolean = synchronized(lock) {
        if (session != activeSession || currentState != AndroidScannerState.PERMISSION_PENDING) return@synchronized false
        currentState = AndroidScannerState.SCANNING
        true
    }

    /** Record denial without starting or retaining camera work. */
    fun permissionDenied(session: Long): Boolean = synchronized(lock) {
        if (session != activeSession || currentState !in setOf(
                AndroidScannerState.PERMISSION_PENDING,
                AndroidScannerState.SCANNING,
            )
        ) return@synchronized false
        currentState = AndroidScannerState.PERMISSION_DENIED
        activeSession = null
        true
    }

    /** Record a camera or decoder failure so the presentation can offer a retry. */
    fun captureFailed(session: Long): Boolean = synchronized(lock) {
        if (session != activeSession || currentState != AndroidScannerState.SCANNING) return@synchronized false
        currentState = AndroidScannerState.CAMERA_ERROR
        activeSession = null
        true
    }

    /** Cancel a permission prompt or active camera session and fence its late callbacks. */
    fun cancel(session: Long): Boolean = synchronized(lock) {
        if (session != activeSession || currentState !in setOf(
                AndroidScannerState.PERMISSION_PENDING,
                AndroidScannerState.SCANNING,
            )
        ) return@synchronized false
        currentState = AndroidScannerState.CANCELLED
        activeSession = null
        true
    }

    /**
     * Submit one frame's raw text to the Rust classifier. Invalid or oversized text is dropped.
     * The state changes before the callback so concurrent duplicate frames cannot be forwarded.
     */
    fun onScanText(session: Long, text: String): AndroidScanResult {
        val result = synchronized(lock) {
            if (session != activeSession) return@synchronized AndroidScanResult.STALE_SESSION
            if (currentState == AndroidScannerState.SUBMITTED) return@synchronized AndroidScanResult.DUPLICATE
            if (currentState != AndroidScannerState.SCANNING) return@synchronized AndroidScanResult.NOT_SCANNING
            when {
                text.isEmpty() -> AndroidScanResult.EMPTY
                !fitsUtf8Limit(text) -> AndroidScanResult.TOO_LARGE
                else -> {
                    currentState = AndroidScannerState.SUBMITTED
                    AndroidScanResult.SUBMITTED_TO_RUST
                }
            }
        }
        if (result == AndroidScanResult.SUBMITTED_TO_RUST) classifyWithRust(text)
        return result
    }

    /** Close the presentation boundary and ignore every outstanding camera callback. */
    fun close() = synchronized(lock) {
        currentState = AndroidScannerState.CLOSED
        activeSession = null
    }

    private fun beginPermissionRequest(): Long {
        nextSession += 1
        activeSession = nextSession
        currentState = AndroidScannerState.PERMISSION_PENDING
        return nextSession
    }

    private fun fitsUtf8Limit(text: String): Boolean {
        if (text.length > maxUtf8Bytes) return false
        var bytes = 0
        var index = 0
        while (index < text.length) {
            val char = text[index]
            when {
                char.code <= 0x7f -> bytes += 1
                char.code <= 0x7ff -> bytes += 2
                Character.isHighSurrogate(char) -> {
                    if (index + 1 >= text.length || !Character.isLowSurrogate(text[index + 1])) return false
                    bytes += 4
                    index += 1
                }
                Character.isLowSurrogate(char) -> return false
                else -> bytes += 3
            }
            if (bytes > maxUtf8Bytes) return false
            index += 1
        }
        return true
    }

    private companion object {
        const val MAX_SCAN_BYTES = 2048
    }
}
