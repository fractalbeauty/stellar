package net.trillia.stellar.desktop

import androidx.compose.runtime.Composable
import androidx.compose.runtime.key
import androidx.compose.ui.tooling.preview.Preview
import net.trillia.stellar.PreviewWrapper
import net.trillia.stellar.desktop.inspector.FieldContent
import net.trillia.stellar.desktop.inspector.InspectorFieldNumber
import net.trillia.stellar.desktop.inspector.InspectorFieldText
import uniffi.stellar_graph.AttributeSchema
import uniffi.stellar_graph.Value
import uniffi.stellar_graph.ValueKind

/** Inspector field for a graph attribute, given an [attributeSchema]. */
@Composable
internal fun EntityInspectorField(
    fieldKey: Any,
    attributeSchema: AttributeSchema,
    values: Collection<Value?>,
    onSet: (Value) -> Unit,
) = EntityInspectorField(fieldKey, attributeSchema.name, attributeSchema.value, values, onSet)

/**
 * Inspector field for a graph value.
 */
@Composable
internal fun EntityInspectorField(
    fieldKey: Any,
    label: String,
    valueKind: ValueKind,
    values: Collection<Value?>,
    onSet: (Value) -> Unit,
) {
    key(fieldKey) {
        when (valueKind) {
            ValueKind.NUMBER -> {
                InspectorFieldNumber(
                    label = label,
                    content = attributeContent(values = values, valueKind = valueKind) { (it as? Value.Number)?.v1 },
                    onSave = { number -> onSet(number?.let { Value.Number(it) } ?: Value.None) },
                )
            }

            ValueKind.TEXT -> {
                InspectorFieldText(
                    label = label,
                    content = attributeContent(values = values, valueKind = valueKind) { (it as? Value.Text)?.v1 },
                    onSave = { text -> onSet(Value.Text(text)) },
                )
            }

            // Bytes can't be edited
            ValueKind.BYTES -> {
                InspectorFieldText(
                    label = label,
                    content =
                        attributeContent(values = values, valueKind = valueKind) { (it as? Value.Bytes)?.let { BYTES_TEXT } },
                    onSave = null,
                )
            }
        }
    }
}

private const val BYTES_TEXT = "<bytes>"

internal fun <T> attributeContent(
    values: Collection<Value?>,
    valueKind: ValueKind,
    getValueOrNull: (Value) -> T?,
): FieldContent<T> {
    val distinctValues = values.filterNotNull().filter { it != Value.None }.distinct()
    return when (distinctValues.size) {
        0 -> {
            FieldContent.None
        }

        1 -> {
            val value = distinctValues.single()
            getValueOrNull(value)?.let { FieldContent.Value(it) }
                ?: FieldContent.Invalid(value.displayText(), "expected ${valueKind.name.lowercase()}")
        }

        else -> {
            FieldContent.Mixed
        }
    }
}

private fun Value.displayText(): String =
    when (this) {
        is Value.Number -> v1.toString()
        is Value.Text -> "\"$v1\""
        is Value.Bool -> v1.toString()
        is Value.Bytes -> BYTES_TEXT
        Value.None -> ""
    }

@Composable
private fun PreviewTemplate(
    label: String,
    valueKind: ValueKind,
    vararg values: Value?,
) {
    PreviewWrapper(padded = true) {
        EntityInspectorField(
            fieldKey = label,
            label = label,
            valueKind = valueKind,
            values = values.toList(),
            onSet = {},
        )
    }
}

@Composable
@Preview
private fun `Preview EntityInspectorField Bytes`() {
    PreviewTemplate(
        label = "Bytes",
        valueKind = ValueKind.BYTES,
        Value.Bytes(byteArrayOf(1, 2)),
    )
}

@Composable
@Preview
private fun `Preview EntityInspectorField Number`() {
    PreviewTemplate(
        label = "Number",
        valueKind = ValueKind.NUMBER,
        Value.Number(180.0),
    )
}

@Composable
@Preview
private fun `Preview EntityInspectorField Number with invalid value`() {
    PreviewTemplate(
        label = "Number",
        valueKind = ValueKind.NUMBER,
        Value.Text("abc"),
    )
}

@Composable
@Preview
private fun `Preview EntityInspectorField Text`() {
    PreviewTemplate(
        label = "Text",
        valueKind = ValueKind.TEXT,
        Value.Text("Song Title"),
    )
}

@Composable
@Preview
private fun `Preview EntityInspectorField Text with mixed values`() {
    PreviewTemplate(
        label = "Text",
        valueKind = ValueKind.TEXT,
        Value.Text("A"),
        Value.Text("B"),
    )
}

@Composable
@Preview
private fun `Preview EntityInspectorField Text with no value`() {
    PreviewTemplate(
        label = "Text",
        valueKind = ValueKind.TEXT,
        null,
    )
}
