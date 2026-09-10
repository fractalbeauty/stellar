package net.trillia.stellar

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateSetOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import net.trillia.stellar.desktop.EntityTable
import net.trillia.stellar.desktop.rememberEntityTableState
import uniffi.stellar.Core
import uniffi.stellar_graph.EntityKind
import kotlin.collections.component1
import kotlin.collections.component2

@Composable
fun InspectPane(
    core: Core,
    schemaManager: SchemaManager,
    selectedEntity: EntityKind,
) {
    val schemaNullable by schemaManager.schemaState.collectAsState()
    val schema = schemaNullable ?: return

    val state = rememberEntityTableState(core, schema, entityKind = selectedEntity)

    val selected = remember(selectedEntity) { mutableStateSetOf<EntityId>() }

    Column {
        EntityTable(
            state = state,
            selected = selected,
        )

        if (selected.isNotEmpty()) {
//            Inspector(selected)
        }
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
