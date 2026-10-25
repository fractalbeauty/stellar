package net.trillia.stellar.desktop.inspector

import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState

sealed interface ParseResult<out T> {
    data class Success<T>(
        val value: T,
    ) : ParseResult<T>

    data object Failure : ParseResult<Nothing>
}

/**
 * State for a text input for editing [savedValue]. [formatValue] and [parseValue] convert from text
 * to the saved type.
 */
class FieldDraft<T>(
    private var savedValue: T,
    private val onSave: (T) -> Unit,
    private val formatValue: (T) -> String,
    private val parseValue: (String) -> ParseResult<T>,
) {
    val textFieldState = TextFieldState(formatValue(savedValue))

    private val text: String get() = textFieldState.text.toString()
    private var focused = false

    fun handleFocusChanged(newFocused: Boolean) {
        val prevFocused = focused
        focused = newFocused
        if (prevFocused && !newFocused) {
            save()
        }
    }

    fun handleSavedValueChanged(newSavedValue: T) {
        val unchanged = text == formatValue(savedValue)
        savedValue = newSavedValue
        if (!focused || unchanged) {
            reset(formatValue(newSavedValue))
        }
    }

    fun save() {
        when (val parsed = parseValue(text)) {
            is ParseResult.Success -> {
                if (parsed.value != savedValue) {
                    onSave(parsed.value)
                }
                reset(formatValue(parsed.value))
            }

            ParseResult.Failure -> {
                reset(formatValue(savedValue))
            }
        }
    }

    fun cancel() {
        reset(formatValue(savedValue))
    }

    private fun reset(newText: String) {
        if (newText != text) textFieldState.setTextAndPlaceCursorAtEnd(newText)
    }
}

/**
 * Remembers a [FieldDraft] for the field's lifetime, keeping it up to date with [savedValue].
 */
@Composable
fun <T> rememberFieldDraft(
    savedValue: T,
    onSave: (T) -> Unit,
    formatValue: (T) -> String,
    parseValue: (String) -> ParseResult<T>,
): FieldDraft<T> {
    val updatedOnSave by rememberUpdatedState(onSave)
    val updatedFormatValue by rememberUpdatedState(formatValue)
    val updatedParseValue by rememberUpdatedState(parseValue)
    val draft =
        remember {
            FieldDraft(
                savedValue,
                onSave = { updatedOnSave(it) },
                formatValue = { updatedFormatValue(it) },
                parseValue = { updatedParseValue(it) },
            )
        }
    LaunchedEffect(savedValue) {
        draft.handleSavedValueChanged(savedValue)
    }
    return draft
}
