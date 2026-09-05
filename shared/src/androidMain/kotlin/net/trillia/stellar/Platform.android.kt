package net.trillia.stellar

import android.icu.text.DecimalFormat
import android.os.Build

class AndroidPlatform : Platform {
    override val name: String = "Android ${Build.VERSION.SDK_INT}"
}

actual fun getPlatform(): Platform = AndroidPlatform()

actual fun formatFloat(
    float: Float,
    decimals: Int,
): String {
    val df = DecimalFormat()
    df.maximumFractionDigits = decimals
    return df.format(float)
}
