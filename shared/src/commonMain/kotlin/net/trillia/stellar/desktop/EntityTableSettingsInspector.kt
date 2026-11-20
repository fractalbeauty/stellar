package net.trillia.stellar.desktop

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import net.trillia.stellar.FakeHandler
import net.trillia.stellar.PreviewWrapper
import net.trillia.stellar.desktop.inspector.Inspector
import net.trillia.stellar.replaceWhere
import uniffi.stellar_graph.SortDirection

@Composable
fun EntityTableSettingsInspector(entityTable: EntityTableState) {
    Inspector {
        Text("Sort")

        val sortedColumns =
            entityTable.columns.value
                .filter { it.sort != null }
                .sortedBy { it.sort?.first }
        val nextSortOrder = (entityTable.columns.value.maxOfOrNull { it.sort?.first ?: 0 } ?: 0) + 1

        sortedColumns.forEach { tableColumn ->
            SortedColumn(
                tableColumn,
                columns = entityTable.columns.value,
                onReplace = { id ->
                    entityTable.columns.value =
                        entityTable.columns.value
                            .replaceWhere({ it.id == tableColumn.id }) {
                                it.withSort(null)
                            }.replaceWhere({ it.id == id }) {
                                it.withSort(tableColumn.sort)
                            }
                },
                onSortDirectionChange = { sortDirection ->
                    entityTable.columns.value =
                        entityTable.columns.value.replaceWhere({ it.id == tableColumn.id }) {
                            it.withSort((it.sort?.first ?: nextSortOrder) to sortDirection)
                        }
                },
                onRemove = {
                    entityTable.columns.value =
                        entityTable.columns.value.replaceWhere({ it.id == tableColumn.id }) {
                            it.withSort(null)
                        }
                },
            )
        }

        AddSortDropdown(
            entityTable.columns.value,
            onAddSort = { id ->
                entityTable.columns.value =
                    entityTable.columns.value.replaceWhere({ it.id == id }) {
                        it.withSort((it.sort?.first ?: nextSortOrder) to SortDirection.ASCENDING)
                    }
            },
        )

        Button("Clear sort") {
            entityTable.columns.value =
                entityTable.columns.value.map { it.withSort(null) }
        }
    }
}

@Composable
fun SortedColumn(
    tableColumn: EntityTableColumn,
    columns: List<EntityTableColumn>,
    onReplace: (id: String) -> Unit,
    onSortDirectionChange: (SortDirection) -> Unit,
    onRemove: () -> Unit,
) {
    Row {
        ReplaceSortDropdown(tableColumn, columns, onReplace)

        SortOrderDropdown(
            sortDirection = tableColumn.sort?.second ?: SortDirection.ASCENDING,
            onSortDirectionChange = onSortDirectionChange,
        )

        Button(
            "x",
            onClick = onRemove,
        )
    }
}

@Composable
fun ReplaceSortDropdown(
    column: EntityTableColumn,
    columns: List<EntityTableColumn>,
    onReplace: (id: String) -> Unit,
) {
    var isExpanded by remember { mutableStateOf(false) }

    Box {
        Button(column.header, onClick = { isExpanded = true })
        DropdownMenu(expanded = isExpanded, onDismissRequest = { isExpanded = false }) {
            columns.forEach {
                if (it.sort == null && it.id != column.id) {
                    DropdownMenuItem(
                        { Text(it.header) },
                        onClick = {
                            onReplace(it.id)
                            isExpanded = false
                        },
                    )
                }
            }
        }
    }
}

@Composable
fun SortOrderDropdown(
    sortDirection: SortDirection,
    onSortDirectionChange: (SortDirection) -> Unit,
) {
    var isExpanded by remember { mutableStateOf(false) }

    val label =
        when (sortDirection) {
            SortDirection.ASCENDING -> "Ascending"
            SortDirection.DESCENDING -> "Descending"
        }

    Box {
        Button(label, onClick = { isExpanded = true })
        DropdownMenu(expanded = isExpanded, onDismissRequest = { isExpanded = false }) {
            DropdownMenuItem(
                { Text("Ascending") },
                onClick = {
                    onSortDirectionChange(SortDirection.ASCENDING)
                    isExpanded = false
                },
            )
            DropdownMenuItem(
                { Text("Descending") },
                onClick = {
                    onSortDirectionChange(SortDirection.DESCENDING)
                    isExpanded = false
                },
            )
        }
    }
}

@Composable
fun AddSortDropdown(
    columns: List<EntityTableColumn>,
    onAddSort: (id: String) -> Unit,
) {
    var isExpanded by remember { mutableStateOf(false) }

    Box {
        Button("Add sort", onClick = { isExpanded = true })
        DropdownMenu(expanded = isExpanded, onDismissRequest = { isExpanded = false }) {
            columns.forEach {
                if (it.sort == null) {
                    DropdownMenuItem(
                        { Text(it.header) },
                        onClick = {
                            onAddSort(it.id)
                            isExpanded = false
                        },
                    )
                }
            }
        }
    }
}

@Composable
@Preview
fun `Preview AddSortDropdown Default`() {
    PreviewWrapper(padded = true) {
        Box(Modifier.size(300.dp)) {
            AddSortDropdown(
                columns = emptyList(), // TODO
                onAddSort = FakeHandler("onAddSort"),
            )
        }
    }
}

@Composable
@Preview
fun `Preview SortOrderDropdown Default`() {
    PreviewWrapper(padded = true) {
        Box(Modifier.size(300.dp)) {
            SortOrderDropdown(
                sortDirection = SortDirection.ASCENDING,
                onSortDirectionChange = FakeHandler("onSortDirectionChange"),
            )
        }
    }
}
