package net.trillia.stellar.mobile

import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MobileTheme(content: @Composable () -> Unit) {
    MaterialTheme {
        content()
    }
}
