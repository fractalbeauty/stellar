package net.trillia.stellar

import java.text.DecimalFormat

class JVMPlatform : Platform {
    override val name: String = "Java ${System.getProperty("java.version")}"
}

actual fun getPlatform(): Platform = JVMPlatform()

actual fun formatFloat(
    float: Float,
    decimals: Int,
): String {
    val df = DecimalFormat()
    df.maximumFractionDigits = decimals
    return df.format(float)
}
