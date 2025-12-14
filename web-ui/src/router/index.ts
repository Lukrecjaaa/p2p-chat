/**
 * @file index.ts
 * @brief Defines the application's routes using Vue Router.
 * This file configures the navigation paths and their corresponding components.
 */
import { createRouter, createWebHistory } from 'vue-router'
import MessengerView from '../views/MessengerView.vue'

/**
 * Creates and configures the Vue Router instance.
 * @type {Router}
 */
const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    /**
     * Defines the main messenger route.
     * @property {string} path - The URL path for this route.
     * @property {string} name - The name of the route, used for programmatic navigation.
     * @property {VueComponent} component - The component to render when this route is active.
     */
    {
      path: '/',
      name: 'messenger',
      component: MessengerView,
    },
  ],
})

export default router
