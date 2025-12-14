<script setup lang="ts">
/**
 * @file App.vue
 * @description The main application component, acting as the root Vue component.
 * It primarily renders the current route's component using Vue Router's <RouterView>.
 */
import { RouterView } from 'vue-router'
</script>

<template>
  <!--
    @component
    @description Renders the component mapped to the current route.
    This is the primary outlet for displaying different views of the application.
  -->
  <RouterView />
</template>

<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

body {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

#app {
  width: 100vw;
  height: 100vh;
  overflow: hidden;
}
</style>
