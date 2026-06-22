package net.trillia.stellar

import android.content.Context

object RustNdkContext {
    init {
        System.loadLibrary("stellar")
    }

    external fun init(context: Context)
}
