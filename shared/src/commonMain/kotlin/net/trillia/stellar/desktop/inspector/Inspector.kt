package net.trillia.stellar.desktop.inspector

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

internal val inspectorFieldLabelHeight = 24.dp
internal val inspectorFieldHeight = 32.dp
internal val inspectorFieldWidth = 200.dp

@Composable
fun Inspector(
    modifier: Modifier = Modifier,
    fields: @Composable ColumnScope.() -> Unit,
) {
    Column(modifier.padding(8.dp).width(250.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        fields()
    }
}
