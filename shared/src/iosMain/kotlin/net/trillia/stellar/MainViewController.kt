package net.trillia.stellar

import androidx.compose.runtime.MutableState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.window.ComposeUIViewController
import kotlinx.coroutines.DelicateCoroutinesApi
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.launch
import platform.UIKit.UIViewController
import uniffi.stellar.Core

fun MainViewController(): UIViewController {
    val devicesManager = DevicesManager()
    val schemaManager = SchemaManager()

    val coreState: MutableState<Core?> = mutableStateOf(null)
    @OptIn(DelicateCoroutinesApi::class)
    GlobalScope.launch {
        coreState.value = Core.spawn(profile = "default", devicesChangeHandler = devicesManager, schemaChangeHandler = schemaManager)
    }

    return ComposeUIViewController {
        val core by coreState

        core?.let { core ->
            App(core = core, devicesManager = devicesManager, schemaManager = schemaManager)
        }
    }
}
