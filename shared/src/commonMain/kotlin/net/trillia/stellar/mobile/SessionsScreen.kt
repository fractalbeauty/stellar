package net.trillia.stellar.mobile

import androidx.compose.foundation.layout.Column
import androidx.compose.material3.Button
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.lifecycle.viewmodel.compose.viewModel
import uniffi.stellar.Core

@Composable
fun SessionsScreen(core: Core) {
    val viewModel = viewModel { SessionsViewModel(core = core) }
    val uiState by viewModel.uiState.collectAsState()

    Column {
        Text("verification uri: ${uiState.verificationUriComplete}")
        Button(
            onClick = { viewModel.startDeviceCodeFlow() },
        ) {
            Text("log in")
        }
    }
}
