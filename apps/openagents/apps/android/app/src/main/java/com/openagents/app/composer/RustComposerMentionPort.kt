// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.composer

import uniffi.openagents_core.CoreClient
import uniffi.openagents_core.fileMentionLink

/** Binds composer mention search and link encoding to existing Rust APIs. */
class RustComposerMentionPort(
    private val client: CoreClient,
    private val deviceId: String,
    private val chatId: String,
    private val spaceId: String? = null,
) : ComposerMentionPort {
    override suspend fun searchFiles(query: String): List<ComposerMentionFile> =
        client.searchFiles(deviceId, chatId, spaceId, query)
            .take(MAX_RESULTS)
            .map { match -> ComposerMentionFile(match.path, match.isDir) }

    override fun fileMentionLink(file: ComposerMentionFile): String =
        fileMentionLink(file.path, file.isDirectory)

    private companion object {
        const val MAX_RESULTS = 5
    }
}
