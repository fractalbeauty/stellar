package net.trillia.stellar.desktop.table

import androidx.compose.runtime.Composable
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

data class TableColumnDefinition<Row, Value>(
    val id: String,
    val header: String,
    val initialWidth: Dp,
    val minWidth: Dp = 40.dp,
    val accessor: (Row) -> Value,
    val renderer: @Composable (value: Value) -> Unit,
)
