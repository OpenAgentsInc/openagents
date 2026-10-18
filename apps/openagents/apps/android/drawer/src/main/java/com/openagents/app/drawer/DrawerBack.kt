package com.openagents.app.drawer

import android.app.Activity
import android.os.Build
import android.view.View
import android.window.BackEvent
import android.window.OnBackAnimationCallback
import android.window.OnBackInvokedCallback
import android.window.OnBackInvokedDispatcher

/** The shell installs this only while the drawer is open and closes it on teardown. */
object DrawerBack {
    fun install(activity: Activity, drawer: View, closeDrawer: () -> Unit): AutoCloseable {
        return when {
            Build.VERSION.SDK_INT >= 34 -> Animated.install(activity, drawer, closeDrawer)
            Build.VERSION.SDK_INT >= 33 -> Invoked.install(activity, closeDrawer)
            else -> AutoCloseable { } // Older shells call handleBack from their back callback.
        }
    }

    fun handleBack(drawerOpen: Boolean, closeDrawer: () -> Unit): Boolean {
        if (!drawerOpen) return false
        closeDrawer()
        return true
    }

    @android.annotation.TargetApi(33)
    private object Invoked {
        fun install(activity: Activity, close: () -> Unit): AutoCloseable {
            val callback = OnBackInvokedCallback { close() }
            activity.onBackInvokedDispatcher.registerOnBackInvokedCallback(OnBackInvokedDispatcher.PRIORITY_OVERLAY, callback)
            return AutoCloseable { activity.onBackInvokedDispatcher.unregisterOnBackInvokedCallback(callback) }
        }
    }

    @android.annotation.TargetApi(34)
    private object Animated {
        fun install(activity: Activity, drawer: View, close: () -> Unit): AutoCloseable {
            val callback = object : OnBackAnimationCallback {
                override fun onBackStarted(event: BackEvent) = Unit
                override fun onBackProgressed(event: BackEvent) {
                    drawer.translationX = -drawer.width * event.progress
                }
                override fun onBackCancelled() { drawer.translationX = 0f }
                override fun onBackInvoked() { drawer.translationX = 0f; close() }
            }
            activity.onBackInvokedDispatcher.registerOnBackInvokedCallback(OnBackInvokedDispatcher.PRIORITY_OVERLAY, callback)
            return AutoCloseable {
                activity.onBackInvokedDispatcher.unregisterOnBackInvokedCallback(callback)
                drawer.translationX = 0f
            }
        }
    }
}
