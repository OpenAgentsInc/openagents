package com.openagents.app.drawer

import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import kotlinx.coroutines.yield
import org.junit.Assert.*
import org.junit.Test

class DrawerPresenterTest {
    private val live = DrawerKey(DrawerOrigin.LIVE, "same-id")
    private val saved = DrawerKey(DrawerOrigin.SAVED, "same-id")
    private fun row(key: DrawerKey, title: String) = DrawerRow(key, title, "Offline computer", "Project",
        if (key.origin == DrawerOrigin.LIVE) "My agent · custom agent · Runtime" else null,
        if (key.origin == DrawerOrigin.LIVE) "Waiting for you" else null,
        if (key.origin == DrawerOrigin.LIVE) true else null,
        false, false, setOf(DrawerAction.PIN))
    private fun frame() = DrawerFrame(listOf(row(saved, "Saved"), row(live, "Live")), live, true)

    /** Scripted frames only: this fixture does not implement history/search/mutations. */
    private class Fixture(var answer: DrawerFrame) : DrawerCore {
        val requests = mutableListOf<DrawerRequest>()
        val intents = mutableListOf<DrawerIntent>()
        val restores = mutableListOf<DrawerKey>()
        var failure: Exception? = null
        override suspend fun load(request: DrawerRequest): DrawerFrame {
            requests += request
            failure?.let { throw it }
            return answer
        }
        override suspend fun send(intent: DrawerIntent) { intents += intent }
        override suspend fun restore(key: DrawerKey) { restores += key }
    }

    @Test fun rows_selection_agent_status_and_unread_stay_exactly_as_core_supplied() = runBlocking {
        val fixture = Fixture(frame())
        val frames = mutableListOf<DrawerFrame>()
        val presenter = DrawerPresenter(fixture, this, frames::add)
        presenter.show()
        yield()
        assertSame(fixture.answer, frames.single())
        assertEquals(listOf(saved, live), frames.single().rows.map { it.key })
        assertEquals(live, frames.single().selected)
        assertEquals("My agent · custom agent · Runtime", frames.single().rows[1].agentLine)
        assertEquals(true, frames.single().rows[1].unread)
        assertNull(frames.single().rows[0].agentLine)
        assertNull(frames.single().rows[0].status)
        assertNull(frames.single().rows[0].unread)
    }

    @Test fun mutation_and_raw_search_are_forwarded_without_local_ranking_or_optimism() = runBlocking {
        val fixture = Fixture(frame())
        val frames = mutableListOf<DrawerFrame>()
        val presenter = DrawerPresenter(fixture, this, frames::add)
        val request = DrawerRequest(DrawerTab.SAVED, "  🦀 Exact Query  ", projectId = "shared-project")
        presenter.show(request)
        yield()
        val intent = DrawerIntent.Pin(saved, true)
        presenter.send(intent)
        yield()
        assertEquals(listOf(request, request), fixture.requests)
        assertEquals(listOf(intent), fixture.intents)
        assertFalse(frames.last().rows[0].pinned)
        assertEquals(listOf(saved, live), frames.last().rows.map { it.key })
    }

    @Test fun restore_resolves_one_existing_key_without_creating_a_session() = runBlocking {
        val fixture = Fixture(frame())
        val presenter = DrawerPresenter(fixture, this) { }
        presenter.restore(saved, DrawerRequest(DrawerTab.SAVED))
        yield()
        assertEquals(listOf(saved), fixture.restores)
        assertTrue(fixture.intents.isEmpty())
        assertEquals(1, fixture.requests.size)
    }

    @Test fun failure_and_account_detach_do_not_leave_old_rows_visible() = runBlocking {
        val fixture = Fixture(frame())
        val frames = mutableListOf<DrawerFrame>()
        val presenter = DrawerPresenter(fixture, this, frames::add)
        presenter.show()
        yield()
        fixture.failure = IllegalStateException("synthetic refusal")
        presenter.send(DrawerIntent.Archive(saved, true))
        yield()
        assertTrue(frames.last().rows.isEmpty())
        assertNull(frames.last().selected)
        assertNotNull(frames.last().error)
        presenter.detach()
        presenter.show()
        yield()
        assertTrue(frames.last().rows.isEmpty())
        assertEquals(2, fixture.requests.size)
    }

    @Test fun late_noncooperative_old_query_cannot_replace_a_new_frame() = runBlocking {
        val old = CompletableDeferred<DrawerFrame>()
        val fixture = object : DrawerCore {
            override suspend fun load(request: DrawerRequest): DrawerFrame = if (request.query == "old") {
                withContext(NonCancellable) { old.await() }
            } else DrawerFrame(emptyList(), null, false)
            override suspend fun send(intent: DrawerIntent) = Unit
            override suspend fun restore(key: DrawerKey) = Unit
        }
        val frames = mutableListOf<DrawerFrame>()
        val presenter = DrawerPresenter(fixture, this, frames::add)
        presenter.show(DrawerRequest(query = "old"))
        yield()
        presenter.show(DrawerRequest(query = "new"))
        yield()
        old.complete(frame())
        yield()
        assertTrue(frames.all { it.rows.isEmpty() })
    }

    @Test fun oversized_or_ambiguous_frames_are_refused() {
        assertThrows(IllegalArgumentException::class.java) {
            DrawerFrame(List(5_001) { row(DrawerKey(DrawerOrigin.LIVE, "$it"), "Chat") }, null, false)
        }
        assertThrows(IllegalArgumentException::class.java) {
            DrawerFrame(listOf(row(live, "One"), row(live, "Two")), null, false)
        }
    }
}
