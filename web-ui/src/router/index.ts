import { createRouter, createWebHistory } from 'vue-router'
import MessengerView from '../views/MessengerView.vue'

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: '/',
      name: 'messenger',
      component: MessengerView,
    },
  ],
})

export default router
