package com.avilaops.aad

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

/** AAD is validated before this renderer is constructed. Navigation is local to the app. */
@Composable
fun AADAppView(app: AADApp) {
    val path = remember(app.appId) { mutableStateListOf(app.navigation.initialRoute) }
    BackHandler(enabled = path.size > 1) { path.removeAt(path.lastIndex) }
    val screen = app.screens.firstOrNull { it.route == path.last() } ?: return
    MaterialTheme {
        Column(
            modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(20.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text(screen.title, style = MaterialTheme.typography.headlineMedium)
            screen.components.forEach { component ->
                when (component) {
                    is AADComponent.Text -> Text(component.value)
                    is AADComponent.Button -> Button(
                        onClick = {
                            val action = component.action
                            if (action is AADAction.Navigate &&
                                app.screens.any { it.route == action.target }
                            ) path.add(action.target)
                        }
                    ) { Text(component.label) }
                    is AADComponent.Spacer -> Spacer(Modifier.height(16.dp))
                }
            }
        }
    }
}
