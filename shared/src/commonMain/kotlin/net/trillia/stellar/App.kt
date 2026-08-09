package net.trillia.stellar

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeContentPadding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.LocalMinimumInteractiveComponentEnforcement
import androidx.compose.material3.LocalMinimumInteractiveComponentSize
import androidx.compose.material3.LocalRippleConfiguration
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalFontFamilyResolver
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import net.trillia.stellar.mobile.SessionsScreen
import uniffi.stellar.Core

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun App(core: Core) {
    MaterialTheme {
        Column(
            modifier =
                Modifier
                    .background(Color(0xFFFAFAFA))
                    .safeContentPadding()
                    .fillMaxSize()
                    .padding(16.dp),
//            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
//            SessionsScreen(core = core)

            CompositionLocalProvider(LocalRippleConfiguration provides null) {
                CompositionLocalProvider(LocalMinimumInteractiveComponentSize provides Dp.Unspecified) {
                    InspectPane()
                }
            }
        }
    }
}
