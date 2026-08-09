package net.trillia.stellar

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import uniffi.stellar.DevicesChangeHandler
import uniffi.stellar_sync.DevicesState

class DevicesManager : DevicesChangeHandler {
    private var _devicesState: MutableStateFlow<DevicesState?> = MutableStateFlow(null)

    val devicesState: StateFlow<DevicesState?>
        get() = _devicesState

    override fun onChange(devicesState: DevicesState) {
        _devicesState.value = devicesState
    }
}
