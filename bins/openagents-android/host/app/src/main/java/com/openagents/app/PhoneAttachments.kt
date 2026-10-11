package com.openagents.app

import org.json.JSONObject

/**
 * Rust says whether a chat on screen takes photos or files in the packet's
 * `attachments`: today a web chat on the account surface (#11174); the chat
 * with OpenAgents takes words only. While it is off no picker opens and
 * nothing is sent to attach. Rust checks and bounds what a picker returns.
 */
object PhoneAttachments {
    /** Whether a chat on screen takes photos or files. */
    fun enabled(packet: JSONObject?): Boolean = packet?.optBoolean("attachments") == true

    /** Whether a packet asks for the photo picker: `pick_image`, only while the chat takes images. */
    fun pickRequested(packet: JSONObject): Boolean =
        packet.optString("coder_go") == "pick_image" && enabled(packet)

    /** The kinds the file picker offers: the website's (#11174). */
    val FILE_TYPES = arrayOf("image/png", "image/jpeg", "application/pdf", "text/plain", "text/markdown", "text/*")

    /**
     * Whether one clipboard, drop, or keyboard item may enter the composer:
     * text, never an image or another file (an item naming a URI).
     */
    fun accepts(text: CharSequence?, hasUri: Boolean): Boolean = !hasUri && !text.isNullOrEmpty()
}
