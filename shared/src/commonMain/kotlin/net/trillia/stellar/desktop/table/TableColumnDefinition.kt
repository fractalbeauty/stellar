package net.trillia.stellar.desktop.table

import androidx.compose.runtime.Composable
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import net.trillia.stellar.AttributeKind
import uniffi.stellar_graph.SortDirection

data class TableColumnDefinition<Row, Value>(
    val id: String,
    val header: String,
    val initialWidth: Dp,
    val minWidth: Dp = 40.dp,
    val sort: Pair<Int, SortDirection>?,
    val accessor: (Row) -> Value,
    val renderer: @Composable (value: Value) -> Unit,
)
