package net.trillia.stellar.mobile

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import net.trillia.stellar.DevicesManager
import uniffi.stellar.Core
import uniffi.stellar.CoreException
import uniffi.stellar.logError
import uniffi.stellar_sync.DevicesState
import uniffi.stellar_uniffi.PublicKey
import kotlin.uuid.Uuid

class SessionsViewModel(
    private val core: Core,
    private val devicesManager: DevicesManager,
) : ViewModel() {
    private val _uiState =
        MutableStateFlow(
            SessionsUiState(
                localEndpointId = core.endpointId(),
            ),
        )

    val uiState: StateFlow<SessionsUiState>
        get() = _uiState.asStateFlow()

    init {
        viewModelScope.launch {
            devicesManager.devicesState.collect { devicesState ->
                _uiState.update {
                    it.copy(devicesState = devicesState)
                }
            }
        }
    }

    fun startDeviceCodeFlow() {
        viewModelScope.launch {
            try {
                val verificationUriComplete = core.startDeviceCodeFlow()
                _uiState.update {
                    it.copy(verificationUriComplete = verificationUriComplete)
                }
            } catch (e: CoreException) {
                logError("Error starting device code flow: $e")
            }
        }
    }

    fun revokeAuthSession(session: Uuid) {
        viewModelScope.launch {
            try {
                core.revokeAuthSession(session)
            } catch (e: CoreException) {
                logError("Error revoking session: $e")
            }
        }
    }
}

data class SessionsUiState(
    val verificationUriComplete: String? = null,
    val devicesState: DevicesState? = null,
    val localEndpointId: PublicKey,
)
