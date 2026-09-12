package net.trillia.stellar.desktop.table

import androidx.compose.runtime.Composable
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.unit.dp
import net.trillia.stellar.uiTest
import kotlin.test.Test
import kotlin.test.assertEquals

@OptIn(ExperimentalTestApi::class)
class TableTest {
    @Test
    fun test() =
        uiTest {
            val row1 = 1 to "one"
            val row2 = 2 to "two"
            val row3 = 3 to "three"
            val rows = listOf(row1, row2, row3)

            val columns =
                listOf(
                    TableColumnDefinition<Pair<Int, String>, String>(
                        id = "first",
                        header = "First",
                        initialWidth = 200.dp,
                        sort = null,
                        accessor = { it.first.toString() },
                        renderer = @Composable { TableCellText(it) },
                    ),
                    TableColumnDefinition<Pair<Int, String>, String>(
                        id = "second",
                        header = "Second",
                        initialWidth = 200.dp,
                        sort = null,
                        accessor = { it.second },
                        renderer = @Composable { TableCellText(it) },
                    ),
                )

            var selected: Pair<Int, String>? = null

            setContent {
                Table(
                    data = rows,
                    columns = columns,
                    isRowSelected = { false },
                    onSelectRow = { selected = it },
                    onDeselectRow = { selected = null },
                    onDeselectAllRows = { selected = null },
                    onColumnTap = {},
                )
            }

//            onNodeWithText("one").performClick()

            assertEquals(selected, row1)
        }
}
