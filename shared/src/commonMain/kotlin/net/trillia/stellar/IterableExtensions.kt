package net.trillia.stellar

inline fun <T> Iterable<T>.sumOfFloat(selector: (T) -> Float): Float {
    var sum = 0f
    for (element in this) sum += selector(element)
    return sum
}
