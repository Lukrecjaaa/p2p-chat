/**
 * @file DraggableWindow.vue
 * @brief This component provides a reusable, draggable window element with a title bar
 * and optional controls. It's designed to contain other components, allowing them to be
 * moved around the screen by the user.
 */
<template>
  <!--
    @component DraggableWindow
    @description A reusable, draggable window component with a title bar,
    and optional minimize/close controls. Content is projected via slots.
  -->
  <div
    class="window glass active draggable-window"
    :style="windowStyle"
    v-show="visible"
  >
    <!-- @element title-bar - The draggable title bar of the window. -->
    <div
      class="title-bar"
      @mousedown="startDrag"
    >
      <div class="title-bar-text">
        <!-- @slot title - Content for the window title. Falls back to `title` prop if not provided. -->
        <slot name="title">{{ title }}</slot>
      </div>
      <!-- @element title-bar-controls - Buttons for window actions like minimize and close. -->
      <div class="title-bar-controls">
        <!-- @element minimize-button - Button to minimize the window, shown only if `minimizable` is true. -->
        <button
          v-if="minimizable"
          aria-label="Minimize"
          @click="$emit('minimize')"
        ></button>
        <!-- @element close-button - Button to close the window. -->
        <button
          aria-label="Close"
          @click="$emit('close')"
        ></button>
      </div>
    </div>
    <!-- @element window-body - The main content area of the window. -->
    <div class="window-body has-space">
      <!-- @slot default - Default slot for the main content of the window. -->
      <slot></slot>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'

/**
 * @props
 * @property {string} [title] - The title text displayed in the window's title bar.
 * @property {number} [initialX=100] - The initial X-coordinate (left position) of the window.
 * @property {number} [initialY=100] - The initial Y-coordinate (top position) of the window.
 * @property {boolean} [minimizable=false] - Whether the window should display a minimize button.
 * @property {boolean} [visible=false] - Controls the visibility of the window.
 */
const props = defineProps<{
  title?: string
  initialX?: number
  initialY?: number
  minimizable?: boolean
  visible?: boolean
}>()

/**
 * @emits
 * @event close - Emitted when the close button is clicked.
 * @event minimize - Emitted when the minimize button is clicked.
 */
defineEmits<{
  close: []
  minimize: []
}>()

/**
 * Reactive state for the window's current position.
 * @type {Ref<{x: number, y: number}>}
 */
const position = ref({
  x: props.initialX || 100,
  y: props.initialY || 100
})

/**
 * Reactive state indicating whether the window is currently being dragged.
 * @type {Ref<boolean>}
 */
const dragging = ref(false)
/**
 * Stores the offset from the mouse pointer to the window's top-left corner at the start of a drag.
 * @type {Ref<{x: number, y: number}>}
 */
const dragOffset = ref({ x: 0, y: 0 })

/**
 * Computed style object for positioning and layering the draggable window.
 * @computed
 * @returns {object} CSS style properties.
 */
const windowStyle = computed(() => ({
  position: 'absolute' as const, // Ensures explicit positioning
  left: `${position.value.x}px`,
  top: `${position.value.y}px`,
  zIndex: 1000, // Ensures the window is on top
}))

/**
 * Initiates the drag operation when the title bar is moused down.
 * @function startDrag
 * @param {MouseEvent} e - The mouse event object.
 */
function startDrag(e: MouseEvent) {
  // Prevent dragging if the click originated from a control button
  if ((e.target as HTMLElement).closest('.title-bar-controls')) {
    return
  }

  dragging.value = true
  dragOffset.value = {
    x: e.clientX - position.value.x,
    y: e.clientY - position.value.y
  }

  // Add global event listeners for mousemove and mouseup to handle dragging outside the window
  document.addEventListener('mousemove', onDrag)
  document.addEventListener('mouseup', stopDrag)
  e.preventDefault() // Prevent default browser drag behavior
}

/**
 * Updates the window's position during a drag operation.
 * Constrains the window within the viewport boundaries.
 * @function onDrag
 * @param {MouseEvent} e - The mouse event object.
 */
function onDrag(e: MouseEvent) {
  if (!dragging.value) return

  let newX = e.clientX - dragOffset.value.x
  let newY = e.clientY - dragOffset.value.y

  // Keep window within viewport bounds
  // Approximate window width/height are used as the component doesn't know its own rendered size
  const maxX = window.innerWidth - 300 // Max X position, assuming window width ~300px
  const maxY = window.innerHeight - 200 // Max Y position, assuming window height ~200px

  newX = Math.max(0, Math.min(newX, maxX)) // Ensure X is within [0, maxX]
  newY = Math.max(0, Math.min(newY, maxY)) // Ensure Y is within [0, maxY]

  position.value = { x: newX, y: newY }
}

/**
 * Ends the drag operation and removes the global event listeners.
 * @function stopDrag
 */
function stopDrag() {
  dragging.value = false
  document.removeEventListener('mousemove', onDrag)
  document.removeEventListener('mouseup', stopDrag)
}

/**
 * Lifecycle hook that runs before the component is unmounted.
 * Ensures that event listeners for dragging are cleaned up to prevent memory leaks.
 * @function onUnmounted
 */
onUnmounted(() => {
  document.removeEventListener('mousemove', onDrag)
  document.removeEventListener('mouseup', stopDrag)
})
</script>

<style scoped>
.draggable-window {
  min-width: 300px;
  max-width: 500px;
  user-select: none;
}

.title-bar {
  cursor: move;
}

.window-body {
  background: white;
}
</style>
