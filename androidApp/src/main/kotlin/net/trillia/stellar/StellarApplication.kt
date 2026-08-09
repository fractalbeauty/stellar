package net.trillia.stellar

import android.app.Application
import android.content.Context
import kotlinx.coroutines.DelicateCoroutinesApi
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import uniffi.stellar.Core

class StellarApplication : Application() {
    lateinit var core: Core
        private set
    val coreReady = MutableStateFlow(false)

    var devicesManager = DevicesManager()
    var schemaManager = SchemaManager()

    override fun attachBaseContext(base: Context) {
        super.attachBaseContext(base)

        // Initialize `ndk_context` crate
        RustNdkContext.init(this)
    }

    override fun onCreate() {
        super.onCreate()

        @OptIn(DelicateCoroutinesApi::class)
        GlobalScope.launch {
            core = Core.spawn(profile = "default", devicesChangeHandler = devicesManager, schemaChangeHandler = schemaManager)
            coreReady.value = true
        }
    }
}
