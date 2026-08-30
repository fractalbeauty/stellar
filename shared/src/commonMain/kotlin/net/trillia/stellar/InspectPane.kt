package net.trillia.stellar

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import net.trillia.stellar.desktop.table.Table
import net.trillia.stellar.desktop.table.TableCellText
import net.trillia.stellar.desktop.table.TableColumnDefinition
import org.jetbrains.compose.resources.Font
import stellar.shared.generated.resources.Res
import stellar.shared.generated.resources.tahoma
import uniffi.stellar.Core
import uniffi.stellar_graph.Value
import kotlin.collections.associate

@Composable
fun InspectPane(
    core: Core,
    schemaManager: SchemaManager,
) {
    val entities =
        remember {
            core.getEntities()
        }
    val schemaNullable by schemaManager.schemaState.collectAsState()

    val schema = schemaNullable ?: return

    var selectedEntity by remember { mutableStateOf(schema.importRules.songEntity) }

    val selectedEntitySchema = schema.graph.entities[selectedEntity] ?: return

    val selectedEntityColumns =
        remember(selectedEntitySchema) {
            selectedEntitySchema.attributes.map { it ->
                TableColumnDefinition<Map<String, String>, String>(
                    id = it.value.name,
                    header = it.value.name,
                    initialWidth = 300.dp,
                    accessor = { row -> row[it.value.name].orEmpty() },
                    renderer = { TableCellText(it) },
                )
            }
        }

    val selectedEntityEntities = entities.filterValues { it.kind == selectedEntity }
    val selectedEntityData =
        selectedEntityEntities
            .map {
                it.value.attributes.entries.associate { attributeEntry ->
                    val name = selectedEntitySchema.attributes[attributeEntry.key]?.name ?: attributeEntry.key.toString()
                    val value =
                        when (val attributeValue = attributeEntry.value.value) {
                            is Value.Bytes -> "<bytes>"
                            is Value.Number -> attributeValue.v1.toString()
                            is Value.Text -> attributeValue.v1
                            is Value.Bool -> attributeValue.v1.toString()
                        }
                    name to value
                }
            }

    var selected by remember(selectedEntity) { mutableStateOf<Int?>(null) }

    Column {
        Row {
            schema.graph.entities.forEach {
                Button(it.value.name, onClick = { selectedEntity = it.key })
            }
        }

        Table(selectedEntityData, selectedEntityColumns, selected, { selected = it })

        selected?.let { idx -> selectedEntityData.getOrNull(idx)?.let { Inspector(it) } }
    }
}

@Composable
fun Button(
    label: String,
    isDefault: Boolean = false,
    onClick: () -> Unit,
) {
    val tahoma = FontFamily(Font(Res.font.tahoma))
    val interactionSource = remember { MutableInteractionSource() }
    val isPressed by interactionSource.collectIsPressedAsState()
    val cBgPressed = Color(0xFF000000)
    val cFgPressed = Color(0xFFFFFFFF)
    val cBgBase = Color(0xFFECECEC)
    val cBgHl = Color(0xFFEFEFEF)
    val shape = RoundedCornerShape(5.dp)
    val bg = Brush.verticalGradient(listOf(cBgHl, cBgBase), 4f, 15f)
    Row(
        modifier =
            Modifier
                .clip(shape)
                .widthIn(min = 78.dp)
                .padding(2.dp)
                .border(1.dp, Color(0xFF777777), shape = shape)
                .clickable(onClick = onClick, interactionSource = interactionSource)
                .run({
                    if (isPressed) background(cBgPressed, shape) else background(bg, shape)
                }),
        horizontalArrangement = Arrangement.Center,
    ) {
        Text(
            label,
            fontFamily = tahoma,
            fontSize = 14.sp,
            fontWeight = FontWeight.Medium,
            lineHeight = 1.em,
            modifier = Modifier.padding(vertical = 3.dp, horizontal = 7.dp),
            color = if (isPressed) cFgPressed else Color.Unspecified,
        )
    }
}

val inspectorFieldLabelHeight = 24.dp

@Composable
fun Inspector(obj: Map<String, String>) {
    Column {
        obj.entries.forEach {
            InspectorField(it.key, it.value, false, true)
        }
        // dummy field
        Box(
            contentAlignment = Alignment.Center,
            modifier =
                Modifier
                    .padding(top = inspectorFieldLabelHeight)
                    .height(32.dp)
                    .width(200.dp)
                    .background(Color(0xFFCCCCCC)),
        ) {
            Text("+", fontWeight = FontWeight.Black, color = Color.White, fontSize = 2.em, lineHeight = 1.em)
        }
    }
}

@Composable
private fun InspectorField(
    key: String,
    value: String,
    valIsRef: Boolean,
    valIsEditable: Boolean,
) {
    var valueState = rememberTextFieldState(value) // XXX
    Column {
        Text(key, modifier = Modifier.height(inspectorFieldLabelHeight))
        TextField(
            valueState,
            contentPadding = PaddingValues.Zero,
            colors =
                TextFieldDefaults.colors(
                    unfocusedContainerColor = Color(0xFFEEEEEE),
                    focusedContainerColor = Color(0xFFDDDDDD),
                ),
            modifier = Modifier.height(32.dp).width(200.dp),
        )
    }
}

// @Composable
// @Preview
// fun InspectPanePreview() {
//    Box(modifier = Modifier.padding(16.dp)) {
//        InspectPane()
//    }
// }
