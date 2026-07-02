package net.trillia.stellar.mobile

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import uniffi.stellar.Core
import uniffi.stellar.CoreException
import uniffi.stellar.logError

class SessionsViewModel(
    private val core: Core,
) : ViewModel() {
    private val _uiState = MutableStateFlow(SessionsUiState())

    val uiState: StateFlow<SessionsUiState>
        get() = _uiState.asStateFlow()

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
}

data class SessionsUiState(
    val verificationUriComplete: String? = null,
)
