package net.trillia.stellar.mobile

import androidx.compose.foundation.layout.Column
import androidx.compose.material3.Button
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import kotlinx.coroutines.launch
import uniffi.stellar.Core
import uniffi.stellar.CoreException

@Composable
fun SessionsScreen() {
    val coroutineScope = rememberCoroutineScope()

    var core by remember { mutableStateOf<Core?>(null) }
    LaunchedEffect(Unit) {
        core = Core.spawn()
    }

    Column {
        Text("core: $core")
        Text("auth state: ???")
        Button(
            onClick = {
                coroutineScope.launch {
                    try {
                        val url = core?.startDeviceCodeFlow()
                        print("meow: $url")
                    } catch (e: CoreException) {
                        println("error: ${e.message()}")
                    }
                }
            },
        ) {
            Text("log in")
        }
    }
}
