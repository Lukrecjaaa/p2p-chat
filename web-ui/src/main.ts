/**
 * @file main.ts
 * @brief This is the entry point for the Vue application.
 * It initializes the Vue app, sets up Pinia for state management,
 * Vue Router for navigation, and mounts the application to the DOM.
 */
import './assets/main.css'
import '7.css/dist/7.css'
import '@mdi/font/css/materialdesignicons.css'

import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import router from './router'

/**
 * Creates the main Vue application instance.
 * @type {App}
 */
const app = createApp(App)

/**
 * Initializes the Pinia store for state management.
 * @type {Pinia}
 */
const pinia = createPinia()

app.use(pinia)
app.use(router)

app.mount('#app')
