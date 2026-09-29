// Nearby pairing listens for computers over mDNS (_openagents._udp), which
// Android delivers to an app only while it holds a Wi-Fi multicast lock.
// Rust says when it listens (the packet's `nearby_listening`, set while the
// nearby list is open and the app is in front); this holds the lock exactly
// then and never otherwise, since a held lock costs battery.
package com.openagents.app

import android.content.Context
import android.net.wifi.WifiManager

class NearbyMulticast(context: Context) {
    private val lock: WifiManager.MulticastLock? =
        context.applicationContext.getSystemService(WifiManager::class.java)
            ?.createMulticastLock("openagents-nearby")
            ?.apply { setReferenceCounted(false) }

    /** Holds the lock while `listening`, and releases it otherwise. */
    fun hold(listening: Boolean) {
        val lock = lock ?: return
        runCatching {
            if (listening && !lock.isHeld) lock.acquire()
            if (!listening && lock.isHeld) lock.release()
        }
    }
}
