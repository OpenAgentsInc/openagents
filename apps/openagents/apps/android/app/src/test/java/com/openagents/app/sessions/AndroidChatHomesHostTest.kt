package com.openagents.app.sessions

import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Test
import uniffi.openagents_core.AndroidSessionAction
import uniffi.openagents_core.ChatHomeAction
import uniffi.openagents_core.CoreContinuationDraft
import uniffi.openagents_core.NoHandle

class AndroidChatHomesHostTest {
    @Test fun navigationActionsKeepTheirRustTypes() {
        val navigation = mutableListOf<ChatHomeAction>()
        val drafts = mutableListOf<CoreContinuationDraft>()
        val computer = ChatHomeAction.OpenComputer("computer-chat")
        val saved = ChatHomeAction.OpenSaved("saved-chat")

        dispatchChatHomeAction(computer, { navigation += it }, { drafts += it })
        dispatchChatHomeAction(saved, { navigation += it }, { drafts += it })

        assertEquals(listOf(computer, saved), navigation)
        assertEquals(emptyList<CoreContinuationDraft>(), drafts)
    }

    @Test fun draftWaitsForExplicitContinueAndRoutesRustResult() = runBlocking {
        val draft = CoreContinuationDraft(NoHandle())
        var continued: CoreContinuationDraft? = null
        var routed: AndroidSessionAction? = null

        dispatchChatHomeAction(ChatHomeAction.Draft(draft), {}, { selected ->
            assertSame(draft, selected)
        })
        continueDraftOnSelected(draft, { selected ->
            continued = selected
            AndroidSessionAction.Updated
        }) { action -> routed = action }

        assertSame(draft, continued)
        assertSame(AndroidSessionAction.Updated, routed)
    }
}
