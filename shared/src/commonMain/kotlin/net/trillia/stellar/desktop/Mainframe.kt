package net.trillia.stellar.desktop

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import net.trillia.stellar.AppColors
import net.trillia.stellar.DevicesManager
import net.trillia.stellar.InspectPane
import net.trillia.stellar.SchemaManager
import net.trillia.stellar.mobile.SessionsScreen
import org.jetbrains.compose.resources.Font
import stellar.shared.generated.resources.Res
import stellar.shared.generated.resources.tahoma
import uniffi.stellar.Core
import uniffi.stellar_graph.EntityKind

sealed class ScreenSelection {
    data class Entity(val kind: EntityKind) : ScreenSelection()
    data object Import : ScreenSelection()
    data object Schema : ScreenSelection()
    data object Sessions : ScreenSelection()
}

@Composable
fun Mainframe(
    core: Core,
    devicesManager: DevicesManager,
    schemaManager: SchemaManager,
) {
    var sel: ScreenSelection? by remember { mutableStateOf(null) }
    return Column {
        Topbar()
        Row {
            Sidebar(schemaManager, sel, setSel = { sel = it })
            // And now the screen
            when (sel) {
                is ScreenSelection.Entity -> InspectPane(
                    core = core,
                    schemaManager = schemaManager,
                    (sel as ScreenSelection.Entity).kind
                )

                ScreenSelection.Import -> Importer(core = core)
                ScreenSelection.Schema -> SchemaEditor(core, schemaManager)
                ScreenSelection.Sessions -> SessionsScreen(
                    core = core,
                    devicesManager = devicesManager
                )

                null -> {}
            }
        }
    }
}

@Composable
fun Topbar() {
    return Row(Modifier.fillMaxWidth().height(60.dp).background(Color.hsl(0.0F, 0.0F, 0.9F))) { }
}

@Composable
fun Sidebar(
    schemaManager: SchemaManager, sel: ScreenSelection?, setSel: (ScreenSelection) -> Unit
) {
    val schemaNullable by schemaManager.schemaState.collectAsState()

    val schema = schemaNullable ?: return

    Column(Modifier.fillMaxHeight().width(180.dp).background(Color.hsl(0.0F, 0.0F, 0.8F))) {
        SidebarTitle("Entities")
        schema.graph.entities.forEach {
            SidebarItem(
                it.value.name,
                isSelected = when (sel) {
                    is ScreenSelection.Entity -> sel.kind == it.key
                    else -> false
                },
                onClick = { setSel(ScreenSelection.Entity(it.key)) },
            )
        }
        SidebarTitle("More items")
        SidebarItem(
            "Import", when (sel) {
                is ScreenSelection.Import -> true
                else -> false
            }, onClick = { setSel(ScreenSelection.Import) })
        SidebarItem(
            "Schema", when (sel) {
                is ScreenSelection.Schema -> true
                else -> false
            }, onClick = { setSel(ScreenSelection.Schema) })
        SidebarItem(
            "Sessions", when (sel) {
                is ScreenSelection.Sessions -> true
                else -> false
            }, onClick = { setSel(ScreenSelection.Sessions) })
    }
}

@Composable
fun SidebarTitle(label: String) {
    Text(
        label,
        fontSize = 15.sp,
        fontWeight = FontWeight.Bold,
        color = AppColors.TextSecondary,
        modifier = Modifier.padding(start = 4.dp)
    )
}

@Composable
fun SidebarItem(label: String, isSelected: Boolean, onClick: () -> Unit) {
    val tahoma = FontFamily(Font(Res.font.tahoma))
    val cBg = Color.Transparent
    val bg = Brush.verticalGradient(listOf(AppColors.PrimaryHl, AppColors.Primary), 4f, 15f)

    Row(
        modifier = Modifier.fillMaxWidth().clickable(onClick = onClick).run({
            if (isSelected) background(bg) else background(cBg)
        }),
        horizontalArrangement = Arrangement.Start,
    ) {
        Text(
            label,
            fontFamily = tahoma,
            fontSize = 14.sp,
            fontWeight = FontWeight.Medium,
            lineHeight = 1.em,
            modifier = Modifier.padding(vertical = 3.dp, horizontal = 7.dp),
        )
    }
}