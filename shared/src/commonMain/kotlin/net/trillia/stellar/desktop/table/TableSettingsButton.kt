package net.trillia.stellar.desktop.table

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import net.trillia.stellar.AppColors
import net.trillia.stellar.FakeHandler

@Composable
fun TableSettingsButton(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Box(
        modifier
            .size(tableRowHeight)
            .background(AppColors.TableSettingsButtonBackground)
            .clickable(onClick = onClick),
    )
}

@Composable
@Preview
private fun `Preview TableSettingsButton Default`() {
    TableSettingsButton(FakeHandler("onClick"))
}
