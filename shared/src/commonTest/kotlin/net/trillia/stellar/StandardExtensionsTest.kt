package net.trillia.stellar

import kotlin.test.Test
import kotlin.test.assertEquals

class StandardExtensionsTest {
    @Test
    fun `lower upOrDownTo higher`() {
        val range = 1 upOrDownTo 3

        assertEquals(listOf(1, 2, 3), range.toList())
    }

    @Test
    fun `higher upOrDownTo lower`() {
        val range = 3 upOrDownTo 1

        assertEquals(listOf(3, 2, 1), range.toList())
    }

    @Test
    fun `number upOrDownTo same`() {
        val range = 2 upOrDownTo 2

        assertEquals(listOf(2), range.toList())
    }
}
