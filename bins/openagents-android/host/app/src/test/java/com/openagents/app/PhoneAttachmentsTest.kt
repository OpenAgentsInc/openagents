package com.openagents.app

import org.json.JSONObject
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** The phone chat is text only (#10093). */
class PhoneAttachmentsTest {
    @Test
    fun thePickerOpensOnlyWhileTheChatTakesImages() {
        val off = JSONObject().put("coder_go", "pick_image").put("attachments", false)
        assertFalse(PhoneAttachments.enabled(off))
        assertFalse(PhoneAttachments.pickRequested(off))
        // A packet without the field, as from an older build, is text only too.
        assertFalse(PhoneAttachments.pickRequested(JSONObject().put("coder_go", "pick_image")))
        assertFalse(PhoneAttachments.enabled(null))
        val on = JSONObject().put("coder_go", "pick_image").put("attachments", true)
        assertTrue(PhoneAttachments.pickRequested(on))
        assertFalse(PhoneAttachments.pickRequested(JSONObject().put("coder_go", "wallet").put("attachments", true)))
    }

    @Test
    fun aPastedOrDroppedImageIsRefusedAndTextIsKept() {
        assertTrue(PhoneAttachments.accepts("Look at this", hasUri = false))
        assertFalse(PhoneAttachments.accepts(null, hasUri = true))
        assertFalse(PhoneAttachments.accepts("content://media/external/images/1", hasUri = true))
        assertFalse(PhoneAttachments.accepts(null, hasUri = false))
        assertFalse(PhoneAttachments.accepts("", hasUri = false))
    }
}
