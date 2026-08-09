package net.trillia.stellar.mobile

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import kotlinx.coroutines.launch
import net.trillia.stellar.Button
import net.trillia.stellar.DevicesManager
import uniffi.stellar.Core
import uniffi.stellar.CoreException
import uniffi.stellar.logError
import uniffi.stellar_graph.ValueKind
import kotlin.time.Instant

@Composable
fun SessionsScreen(
    core: Core,
    devicesManager: DevicesManager,
) {
    val viewModel = viewModel { SessionsViewModel(core = core, devicesManager = devicesManager) }
    val uiState by viewModel.uiState.collectAsState()

    val coroutineScope = rememberCoroutineScope()

    Column(modifier = Modifier.padding(16.dp)) {
        Text("Devices")

        Column(modifier = Modifier.padding(start = 16.dp)) {
            val devices =
                uiState.devicesState?.devices

            if (devices == null) {
                Text("Loading")
            } else if (devices.isEmpty()) {
                Text("None")
            }

            devices?.forEach { device ->
                Row {
                    val endpointId = device.endpointId.toHexString().slice(0..6)
                    val lastUsed =
                        device.session?.let { session ->
                            Instant.fromEpochSeconds(session.lastUsedAt.toLong()).toString()
                        } ?: "-- (added manually)"

                    Text("${device.name} ($endpointId..) seen $lastUsed")

                    Button("remove device", onClick = {
                        coroutineScope.launch {
                            try {
                                // TODO
                            } catch (e: CoreException) {
                                logError("$e")
                            }
                        }
                    })
                }
            }
        }

        Text("authed: ${uiState.devicesState?.authed}")

        Text("verification uri: ${uiState.verificationUriComplete}")
        Button(
            "log in to the main frame!!!",
            onClick = { viewModel.startDeviceCodeFlow() },
        )
    }
}
