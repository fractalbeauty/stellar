package net.trillia.stellar.desktop.table

import androidx.compose.runtime.Composable
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.click
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performMouseInput
import androidx.compose.ui.unit.dp
import net.trillia.stellar.performCtrlClick
import net.trillia.stellar.performCtrlShiftClick
import net.trillia.stellar.performShiftClick
import net.trillia.stellar.setContentWithTestProviders
import net.trillia.stellar.uiTest
import kotlin.test.Test
import kotlin.test.assertEquals

@OptIn(ExperimentalTestApi::class)
class TableTest {
    @Test
    fun `selects rows`() =
        uiTest {
            val table = makeTableTest()

            onNodeWithText("one").performClick()
            assertEquals(setOf(row1), table.selected)

            onNodeWithText("two").performClick()
            assertEquals(setOf(row2), table.selected)
        }

    @Test
    fun `selects multiple rows with ctrl`() =
        uiTest {
            val table = makeTableTest()

            onNodeWithText("one").performClick()
            onNodeWithText("three").performCtrlClick()

            assertEquals(setOf(row1, row3), table.selected)
        }

    @Test
    fun `selects a range of rows with shift`() =
        uiTest {
            val table = makeTableTest()

            onNodeWithText("one").performClick()
            onNodeWithText("three").performShiftClick()

            assertEquals(setOf(row1, row2, row3), table.selected)
        }

    @Test
    fun `deselects rows`() =
        uiTest {
            val table = makeTableTest()

            onNodeWithText("one").performClick()
            onNodeWithText("three").performCtrlClick()
            assertEquals(setOf(row1, row3), table.selected)

            onNodeWithText("one").performCtrlClick()
            assertEquals(setOf(row3), table.selected)
        }

    @Test
    fun `extending a shift selection keeps the original anchor`() =
        uiTest {
            val table = makeTableTest()

            onNodeWithText("one").performClick()
            onNodeWithText("three").performShiftClick()
            assertEquals(setOf(row1, row2, row3), table.selected)

            onNodeWithText("two").performShiftClick()
            assertEquals(setOf(row1, row2), table.selected)
        }

    @Test
    fun `ctrl takes precedence over shift`() =
        uiTest {
            val table = makeTableTest()

            onNodeWithText("one").performClick()
            onNodeWithText("three").performCtrlShiftClick()

            assertEquals(setOf(row1, row3), table.selected)
        }

    @Test
    fun `deselects all rows by clicking the background`() =
        uiTest {
            val table = makeTableTest()

            onNodeWithText("one").performClick()
            assertEquals(setOf(row1), table.selected)

            onRoot().performMouseInput { click(Offset(10f, 700f)) }
            assertEquals(emptySet(), table.selected)
        }
}

private val row1 = 1 to "one"
private val row2 = 2 to "two"
private val row3 = 3 to "three"
private val row4 = 4 to "four"
private val rows = listOf(row1, row2, row3, row4)

private val columns =
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

private class TableTestState {
    val selected = mutableSetOf<Pair<Int, String>>()
}

@OptIn(ExperimentalTestApi::class)
private fun ComposeUiTest.makeTableTest(): TableTestState {
    val state = TableTestState()

    setContentWithTestProviders {
        Table(
            data = rows,
            columns = columns,
            isRowSelected = { state.selected.contains(it) },
            onSelectRow = { state.selected.add(it) },
            onDeselectRow = { state.selected.remove(it) },
            onDeselectAllRows = { state.selected.clear() },
            onColumnTap = {},
        )
    }

    return state
}
