package net.trillia.stellar.desktop.inspector

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.tooling.preview.Preview
import net.trillia.stellar.PreviewWrapper

/** An inspector field for text. */
@Composable
fun InspectorFieldText(
    label: String,
    content: FieldContent<String>,
    onSave: ((String) -> Unit)?,
) {
    InspectorFieldOuter(label = label, content = content) { value ->
        InspectorFieldInner(
            label = label,
            savedValue = value.orEmpty(),
            onSave = onSave,
            formatValue = { value ->
                value
            },
            parseValue = { text ->
                ParseResult.Success(text)
            },
        )
    }
}

/** An inspector field for numbers. */
@Composable
fun InspectorFieldNumber(
    label: String,
    content: FieldContent<Double>,
    onSave: ((Double?) -> Unit)?,
) {
    InspectorFieldOuter(label = label, content = content) { value ->
        InspectorFieldInner(
            label = label,
            savedValue = value,
            formatValue = { value ->
                value?.toString().orEmpty()
            },
            parseValue = { text ->
                if (text.isBlank()) {
                    // Map empty text to null
                    ParseResult.Success(null)
                } else {
                    text
                        .toDoubleOrNull()
                        // Filter out NaN and infinity
                        ?.takeIf { it.isFinite() }
                        ?.let { ParseResult.Success(it) }
                        ?: ParseResult.Failure
                }
            },
            onSave = onSave,
        )
    }
}

/** An inspector field showing that multiple values are selected. */
@Composable
private fun InspectorFieldMixed(label: String) {
    Column {
        Text(label, modifier = Modifier.height(inspectorFieldLabelHeight))
        Row(
            modifier = Modifier.height(inspectorFieldHeight).width(inspectorFieldWidth),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text("(multiple values)")
        }
    }
}

/** An inspector field showing that the saved value is invalid. */
@Composable
private fun InspectorFieldInvalid(
    label: String,
    invalid: FieldContent.Invalid,
) {
    Column {
        Text(label, modifier = Modifier.height(inspectorFieldLabelHeight))
        Row(
            modifier = Modifier.height(inspectorFieldHeight).width(inspectorFieldWidth),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                invalid.invalidValue,
            )
        }
        Text(
            invalid.reason,
            color = MaterialTheme.colorScheme.error,
            style = MaterialTheme.typography.bodySmall,
        )
    }
}

@Composable
private fun <T> InspectorFieldOuter(
    label: String,
    content: FieldContent<T>,
    editor: @Composable (value: T?) -> Unit,
) {
    when (content) {
        FieldContent.Mixed -> InspectorFieldMixed(label = label)
        is FieldContent.Invalid -> InspectorFieldInvalid(label = label, invalid = content)
        FieldContent.None -> editor(null)
        is FieldContent.Value -> editor(content.value)
    }
}

/**
 * A text input for editing [savedValue]. [formatValue] and [parseValue] convert from text to the
 * saved type.
 */
@Composable
private fun <T> InspectorFieldInner(
    label: String,
    savedValue: T,
    onSave: ((T) -> Unit)?,
    formatValue: (T) -> String,
    parseValue: (String) -> ParseResult<T>,
) {
    val draft =
        rememberFieldDraft(
            savedValue = savedValue,
            onSave = { onSave?.invoke(it) },
            formatValue = formatValue,
            parseValue = parseValue,
        )
    DisposableEffect(draft) { onDispose { draft.save() } }

    Column {
        Text(label, modifier = Modifier.height(inspectorFieldLabelHeight))
        TextField(
            draft.textFieldState,
            enabled = onSave != null,
            lineLimits = TextFieldLineLimits.SingleLine,
            contentPadding = PaddingValues.Zero,
            colors =
                TextFieldDefaults.colors(
                    unfocusedContainerColor = Color(0xFFEEEEEE),
                    focusedContainerColor = Color(0xFFDDDDDD),
                ),
            modifier =
                Modifier
                    .height(inspectorFieldHeight)
                    .width(inspectorFieldWidth)
                    .onFocusChanged { draft.handleFocusChanged(it.isFocused) }
                    .onPreviewKeyEvent { event ->
                        if (event.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                        when (event.key) {
                            Key.Enter, Key.NumPadEnter -> draft.save()
                            Key.Escape -> draft.cancel()
                            else -> return@onPreviewKeyEvent false
                        }
                        true
                    },
        )
    }
}

@Composable
@Preview
private fun `InspectorFieldText Default`() {
    PreviewWrapper(padded = true) {
        InspectorFieldText(label = "Text", content = FieldContent.Value("Song Title"), onSave = {})
    }
}

@Composable
@Preview
private fun `InspectorFieldText Readonly`() {
    PreviewWrapper(padded = true) {
        InspectorFieldText(label = "Readonly", content = FieldContent.Value("Artist"), onSave = null)
    }
}

@Composable
@Preview
private fun `InspectorFieldText Mixed`() {
    PreviewWrapper(padded = true) {
        InspectorFieldText(label = "Mixed", content = FieldContent.Mixed, onSave = {})
    }
}

@Composable
@Preview
private fun `InspectorFieldNumber Default`() {
    PreviewWrapper(padded = true) {
        InspectorFieldNumber(label = "Number", content = FieldContent.Value(180.0), onSave = {})
    }
}

@Composable
@Preview
private fun `InspectorFieldNumber Empty`() {
    PreviewWrapper(padded = true) {
        InspectorFieldNumber(label = "Empty", content = FieldContent.None, onSave = {})
    }
}

@Composable
@Preview
private fun `InspectorFieldNumber Invalid`() {
    PreviewWrapper(padded = true) {
        InspectorFieldNumber(
            label = "Invalid",
            content = FieldContent.Invalid(invalidValue = "\"abc\"", reason = "expected number"),
            onSave = {},
        )
    }
}
