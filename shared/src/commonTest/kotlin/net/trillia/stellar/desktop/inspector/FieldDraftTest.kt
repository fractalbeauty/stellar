package net.trillia.stellar.desktop.inspector

import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
import kotlin.test.Test
import kotlin.test.assertEquals

class FieldDraftTest {
    private val savedValues = mutableListOf<Double?>()

    private fun assertSaves(vararg expected: Double?) = assertEquals(expected.toList(), savedValues)

    private fun makeNumberDraft() =
        FieldDraft(
            1.0,
            formatValue = { it.toString() },
            parseValue = { text -> text.toDoubleOrNull()?.let { ParseResult.Success(it) } ?: ParseResult.Failure },
            onSave = { savedValues += it },
        )

    private val FieldDraft<*>.text get() = textFieldState.text.toString()

    private fun FieldDraft<*>.focusAndType(newText: String) {
        handleFocusChanged(true)
        textFieldState.setTextAndPlaceCursorAtEnd(newText)
    }

    @Test
    fun `save stores a changed value and normalizes the text`() {
        val draft = makeNumberDraft()
        draft.focusAndType("2")
        draft.save()

        assertSaves(2.0)
        assertEquals("2.0", draft.text)
    }

    @Test
    fun `save normalizes text that parses to the saved value without saving`() {
        val draft = makeNumberDraft()
        draft.focusAndType("1")
        draft.save()

        assertSaves()
        assertEquals("1.0", draft.text)
    }

    @Test
    fun `save reverts unparsable text without saving`() {
        val draft = makeNumberDraft()
        draft.focusAndType("1e")
        draft.save()

        assertSaves()
        assertEquals("1.0", draft.text)
    }

    @Test
    fun `typing does not save until focus is lost`() {
        val draft = makeNumberDraft()
        draft.focusAndType("2")

        assertSaves()

        draft.handleFocusChanged(false)

        assertSaves(2.0)
    }

    @Test
    fun `cancel reverts the edit so losing focus afterwards saves nothing`() {
        val draft = makeNumberDraft()
        draft.focusAndType("2")
        draft.cancel()
        draft.handleFocusChanged(false)

        assertEquals("1.0", draft.text)
        assertSaves()
    }

    @Test
    fun `a new saved value is shown when no edit is in progress`() {
        val draft = makeNumberDraft()
        draft.handleSavedValueChanged(5.0)

        assertEquals("5.0", draft.text)

        draft.handleFocusChanged(true)
        draft.handleSavedValueChanged(6.0)

        assertEquals("6.0", draft.text)
    }

    @Test
    fun `a new saved value does not replace an edit in progress`() {
        val draft = makeNumberDraft()
        draft.focusAndType("2")
        draft.handleSavedValueChanged(5.0)

        assertEquals("2", draft.text)

        draft.handleFocusChanged(false)

        assertSaves(2.0)
    }
}
