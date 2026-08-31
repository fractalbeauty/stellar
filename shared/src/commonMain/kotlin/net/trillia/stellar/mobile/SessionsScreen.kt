package net.trillia.stellar.mobile

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import net.trillia.stellar.desktop.Button
import net.trillia.stellar.DevicesManager
import uniffi.stellar.Core
import uniffi.stellar_sync.DevicesStateDevice
import uniffi.stellar_uniffi.PublicKey
import kotlin.time.Instant
import kotlin.uuid.Uuid

@Composable
fun SessionsScreen(
    core: Core,
    devicesManager: DevicesManager,
) {
    val viewModel = viewModel { SessionsViewModel(core = core, devicesManager = devicesManager) }
    val uiState by viewModel.uiState.collectAsState()

    Column(
        modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(16.dp),
    ) {
        Text("Devices added manually")
        Column(modifier = Modifier.padding(start = 16.dp)) {
            DevicesList(
                uiState.devicesState?.addedDevices,
                localEndpointId = uiState.localEndpointId,
                revokeAuthSession = viewModel::revokeAuthSession,
            )
        }
        Text("Devices from auth")
        Column(modifier = Modifier.padding(start = 16.dp)) {
            DevicesList(
                uiState.devicesState?.authDevices,
                localEndpointId = uiState.localEndpointId,
                revokeAuthSession = viewModel::revokeAuthSession,
            )
        }

        Text("authed: ${uiState.devicesState?.authed}")

        Text("verification uri: ${uiState.verificationUriComplete}")
        Button(
            "log in to the main frame!!!",
            onClick = { viewModel.startDeviceCodeFlow() },
        )
    }
}

@Composable
private fun DevicesList(
    devices: List<DevicesStateDevice>?,
    localEndpointId: PublicKey,
    revokeAuthSession: (Uuid) -> Unit,
) {
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

            val you =
                if (localEndpointId.contentEquals(device.endpointId)) {
                    " (you)"
                } else {
                    ""
                }

            Text("${device.name} ($endpointId..) seen $lastUsed$you")

            device.session?.let { deviceSession ->
                Button("revoke session", onClick = { revokeAuthSession(deviceSession.id) })
            }
        }
    }
}
