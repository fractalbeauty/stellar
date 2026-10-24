package net.trillia.stellar

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp

@Composable
fun PreviewWrapper(
    padded: Boolean = false,
    content: @Composable () -> Unit,
) {
    Box(
        Modifier
            .background(Color.White)
            .runIf(padded) { padding(8.dp) },
    ) {
        content()
    }
}
