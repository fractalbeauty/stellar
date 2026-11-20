package net.trillia.stellar

inline fun <T> List<T>.replaceWhere(
    where: (T) -> Boolean,
    transform: (T) -> T,
): List<T> =
    map { item ->
        if (where(item)) transform(item) else item
    }
