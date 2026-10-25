package net.trillia.stellar.mobile

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.safeContentPadding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import net.trillia.stellar.AppColors
import net.trillia.stellar.DevicesManager
import net.trillia.stellar.SchemaManager
import uniffi.stellar.Core

@Composable
fun MobileApp(
    core: Core,
    devicesManager: DevicesManager,
    schemaManager: SchemaManager,
) {
    MobileTheme {
        Column(
            modifier =
                Modifier
                    .background(AppColors.Background)
                    .safeContentPadding()
                    .fillMaxSize(),
        ) {
            SessionsScreen(core = core, devicesManager = devicesManager)
        }
    }
}
