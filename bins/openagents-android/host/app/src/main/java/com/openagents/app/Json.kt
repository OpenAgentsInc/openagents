package com.openagents.app

import org.json.JSONArray
import org.json.JSONObject

internal fun json(vararg values: Pair<String, Any?>) = JSONObject().apply {
    values.forEach { (key, value) -> put(key, value ?: JSONObject.NULL) }
}

internal fun JSONObject.textOrNull(key: String): String? =
    if (!has(key) || isNull(key)) null else optString(key).takeIf { it.isNotEmpty() }

internal fun JSONObject.objectOrNull(key: String): JSONObject? =
    if (!has(key) || isNull(key)) null else optJSONObject(key)

internal fun JSONArray.objects(): List<JSONObject> = (0 until length()).map { getJSONObject(it) }

/** Parses a packet from Rust and checks its schema and every Rust Native view in it. */
internal fun packet(text: String, schema: String): JSONObject {
    require(text.toByteArray().size in 1..2_097_152) { "OpenAgents returned an unreadable screen." }
    return JSONObject(text).also {
        require(it.getString("schema") == schema) { "This app does not support the returned screen version." }
        for (key in listOf("computers", "coder", "tailnet", "view")) it.objectOrNull(key)?.let { view ->
            require(view.getString("schema") == "rust-native.view.v2") { "Unsupported Rust Native view version." }
        }
    }
}
