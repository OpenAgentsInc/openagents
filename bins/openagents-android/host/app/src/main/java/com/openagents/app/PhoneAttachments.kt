package com.openagents.app

import org.json.JSONObject

/**
 * The phone chat is text only (#10093). Rust says whether the chat takes
 * images in the packet's `attachments` (`coder_tab::ATTACHMENTS_ENABLED`,
 * off since 2026-10-01); while it is off the photo picker never opens,
 * nothing is sent to attach, and the composer takes only text from a paste,
 * a drop, or the keyboard. The shared image pipeline stays in Rust, so
 * turning the switch back on there brings attachments back here.
 */
object PhoneAttachments {
    /** Whether the chat takes images. */
    fun enabled(packet: JSONObject?): Boolean = packet?.optBoolean("attachments") == true

    /** Whether a packet asks for the photo picker: `pick_image`, only while the chat takes images. */
    fun pickRequested(packet: JSONObject): Boolean =
        packet.optString("coder_go") == "pick_image" && enabled(packet)

    /**
     * Whether one clipboard, drop, or keyboard item may enter the composer:
     * text, never an image or another file (an item naming a URI).
     */
    fun accepts(text: CharSequence?, hasUri: Boolean): Boolean = !hasUri && !text.isNullOrEmpty()
}
