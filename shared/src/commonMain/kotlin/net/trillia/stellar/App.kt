package net.trillia.stellar

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.safeContentPadding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.LocalMinimumInteractiveComponentSize
import androidx.compose.material3.LocalRippleConfiguration
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import net.trillia.stellar.desktop.Mainframe
import uniffi.stellar.Core

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun App(
    core: Core,
    devicesManager: DevicesManager,
    schemaManager: SchemaManager,
) {
    MaterialTheme {
        Column(
            modifier =
                Modifier
                    .background(Color(0xFFFAFAFA))
                    .safeContentPadding()
                    .fillMaxSize(),
        ) {
            CompositionLocalProvider(LocalRippleConfiguration provides null) {
                CompositionLocalProvider(LocalMinimumInteractiveComponentSize provides Dp.Unspecified) {
                    Mainframe(core, devicesManager, schemaManager)
                }
            }
        }
    }
}
