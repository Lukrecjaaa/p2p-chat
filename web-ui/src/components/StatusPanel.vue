/**
 * @file StatusPanel.vue
 * @brief This component provides a draggable window that displays real-time
 * system status information. It shows metrics like the number of connected peers,
 * known mailboxes, and pending messages, with periodic updates to keep the
 * information current.
 */
<template>
  <!--
    @component StatusPanel
    @description A draggable window component that displays real-time system status information,
    including connected peers, known mailboxes, and pending messages.
    The data is refreshed periodically.
  -->
  <DraggableWindow
    :initial-x="400"
    :initial-y="50"
    :visible="visible"
    @close="$emit('close')"
  >
    <template #title>
      <!-- @element title-icon - Icon displayed in the panel's title bar. -->
      <img src="/connected-peers.ico" alt="" class="title-icon" />
      <span>System Status</span>
    </template>
    <!-- @section status-content - Displays various system metrics. -->
    <div v-if="status" class="status-content">
      <!-- @element connected-peers-item - Displays the number of connected peers. -->
      <div class="status-item">
        <div class="status-icon">
          <img src="/connected-peers.ico" alt="Connected Peers" />
        </div>
        <div class="status-info">
          <div class="status-label">Connected Peers</div>
          <div class="status-value">{{ status.connected_peers }}</div>
        </div>
      </div>
      <!-- @element known-mailboxes-item - Displays the number of known mailboxes. -->
      <div class="status-item">
        <div class="status-icon">
          <img src="/known-mailboxes.ico" alt="Known Mailboxes" />
        </div>
        <div class="status-info">
          <div class="status-label">Known Mailboxes</div>
          <div class="status-value">{{ status.known_mailboxes }}</div>
        </div>
      </div>
      <!-- @element pending-messages-item - Displays the number of pending messages. -->
      <div class="status-item">
        <div class="status-icon">
          <img src="/pending-messages.ico" alt="Pending Messages" />
        </div>
        <div class="status-info">
          <div class="status-label">Pending Messages</div>
          <div class="status-value">{{ status.pending_messages }}</div>
        </div>
      </div>
    </div>
    <!-- @element status-loading - Displays a loading message if status data is not yet available. -->
    <div v-else class="status-loading">Loading...</div>
  </DraggableWindow>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { getSystemStatus, type SystemStatus } from '@/api/client'
import DraggableWindow from './DraggableWindow.vue'

/**
 * @props
 * @property {boolean} visible - Controls the visibility of the status panel.
 */
defineProps<{
  visible: boolean
}>()

/**
 * @emits
 * @event close - Emitted when the status panel is requested to be closed.
 */
defineEmits<{
  close: []
}>()

/**
 * Reactive state to hold the fetched system status data.
 * @type {Ref<SystemStatus | null>}
 */
const status = ref<SystemStatus | null>(null)
/**
 * Stores the ID of the interval timer used for periodic status updates.
 * @type {number | null}
 */
let intervalId: number | null = null

/**
 * Fetches the current system status from the API and updates the `status` reactive variable.
 * Logs an error if the fetch operation fails.
 * @async
 * @function fetchStatus
 * @returns {Promise<void>}
 */
async function fetchStatus() {
  try {
    status.value = await getSystemStatus()
  } catch (err) {
    console.error('Failed to fetch system status:', err)
  }
}

/**
 * Lifecycle hook: Called after the component has mounted.
 * Initiates the first status fetch and sets up a periodic refresh interval.
 * @function onMounted
 */
onMounted(() => {
  fetchStatus()
  // Update system status every 10 seconds
  intervalId = window.setInterval(fetchStatus, 10000)
})

/**
 * Lifecycle hook: Called before the component unmounts.
 * Clears the periodic status refresh interval to prevent memory leaks.
 * @function onUnmounted
 */
onUnmounted(() => {
  if (intervalId !== null) {
    clearInterval(intervalId)
  }
})
</script>

<style scoped>
.status-content {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.status-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px;
  background: #f8f9fa;
  border-radius: 4px;
  border: 1px solid #e0e0e0;
}

.status-icon {
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.status-icon img {
  width: 32px;
  height: 32px;
  image-rendering: crisp-edges;
}

.status-info {
  flex: 1;
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.status-label {
  font-size: 12px;
  color: #333;
  font-weight: 500;
}

.status-value {
  font-size: 16px;
  font-weight: 700;
  color: #007bff;
}

.status-loading {
  font-size: 12px;
  color: #6c757d;
  text-align: center;
  padding: 16px;
}

.title-icon {
  width: 16px;
  height: 16px;
  vertical-align: middle;
  margin-right: 4px;
  image-rendering: crisp-edges;
}
</style>
