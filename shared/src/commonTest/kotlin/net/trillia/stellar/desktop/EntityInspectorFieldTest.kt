package net.trillia.stellar.desktop

import net.trillia.stellar.desktop.inspector.FieldContent
import uniffi.stellar_graph.Value
import uniffi.stellar_graph.ValueKind
import kotlin.test.Test
import kotlin.test.assertEquals

class EntityInspectorFieldTest {
    private fun testNumberContent(
        expected: FieldContent<Double>,
        vararg values: Value?,
    ) = assertEquals(expected, attributeContent(values.toList(), ValueKind.NUMBER) { (it as? Value.Number)?.v1 })

    @Test
    fun `missing and none values are none`() = testNumberContent(FieldContent.None, null, Value.None)

    @Test
    fun `same values are a single value`() = testNumberContent(FieldContent.Value(1.0), Value.Number(1.0), Value.Number(1.0))

    @Test
    fun `a missing value is ignored when others are set`() = testNumberContent(FieldContent.Value(1.0), Value.Number(1.0), null)

    @Test
    fun `a none value is ignored when others are set`() = testNumberContent(FieldContent.Value(1.0), Value.Number(1.0), Value.None)

    @Test
    fun `different values are mixed`() = testNumberContent(FieldContent.Mixed, Value.Number(1.0), Value.Number(2.0))

    @Test
    fun `different values are mixed even if one is the wrong kind`() =
        testNumberContent(FieldContent.Mixed, Value.Number(1.0), Value.Text("abc"))

    @Test
    fun `a value of another kind is invalid`() = testNumberContent(FieldContent.Invalid("\"abc\"", "expected number"), Value.Text("abc"))
}
