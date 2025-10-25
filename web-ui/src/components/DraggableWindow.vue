<template>
  <div
    class="window glass active draggable-window"
    :style="windowStyle"
    v-show="visible"
  >
    <div
      class="title-bar"
      @mousedown="startDrag"
    >
      <div class="title-bar-text">
        <slot name="title">{{ title }}</slot>
      </div>
      <div class="title-bar-controls">
        <button
          v-if="minimizable"
          aria-label="Minimize"
          @click="$emit('minimize')"
        ></button>
        <button
          aria-label="Close"
          @click="$emit('close')"
        ></button>
      </div>
    </div>
    <div class="window-body has-space">
      <slot></slot>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'

const props = defineProps<{
  title?: string
  initialX?: number
  initialY?: number
  minimizable?: boolean
  visible?: boolean
}>()

defineEmits<{
  close: []
  minimize: []
}>()

const position = ref({
  x: props.initialX || 100,
  y: props.initialY || 100
})

const dragging = ref(false)
const dragOffset = ref({ x: 0, y: 0 })

const windowStyle = computed(() => ({
  position: 'absolute' as const,
  left: `${position.value.x}px`,
  top: `${position.value.y}px`,
  zIndex: 1000,
}))

function startDrag(e: MouseEvent) {
  if ((e.target as HTMLElement).closest('.title-bar-controls')) {
    return
  }

  dragging.value = true
  dragOffset.value = {
    x: e.clientX - position.value.x,
    y: e.clientY - position.value.y
  }

  document.addEventListener('mousemove', onDrag)
  document.addEventListener('mouseup', stopDrag)
  e.preventDefault()
}

function onDrag(e: MouseEvent) {
  if (!dragging.value) return

  let newX = e.clientX - dragOffset.value.x
  let newY = e.clientY - dragOffset.value.y

  // Keep window within viewport bounds
  const maxX = window.innerWidth - 300 // Approximate window width
  const maxY = window.innerHeight - 200 // Approximate window height

  newX = Math.max(0, Math.min(newX, maxX))
  newY = Math.max(0, Math.min(newY, maxY))

  position.value = { x: newX, y: newY }
}

function stopDrag() {
  dragging.value = false
  document.removeEventListener('mousemove', onDrag)
  document.removeEventListener('mouseup', stopDrag)
}

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
