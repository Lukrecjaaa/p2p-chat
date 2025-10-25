<template>
  <DraggableWindow
    :initial-x="400"
    :initial-y="50"
    :visible="visible"
    @close="$emit('close')"
  >
    <template #title>
      <img src="/connected-peers.ico" alt="" class="title-icon" />
      <span>System Status</span>
    </template>
    <div v-if="status" class="status-content">
      <div class="status-item">
        <div class="status-icon">
          <img src="/connected-peers.ico" alt="Connected Peers" />
        </div>
        <div class="status-info">
          <div class="status-label">Connected Peers</div>
          <div class="status-value">{{ status.connected_peers }}</div>
        </div>
      </div>
      <div class="status-item">
        <div class="status-icon">
          <img src="/known-mailboxes.ico" alt="Known Mailboxes" />
        </div>
        <div class="status-info">
          <div class="status-label">Known Mailboxes</div>
          <div class="status-value">{{ status.known_mailboxes }}</div>
        </div>
      </div>
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
    <div v-else class="status-loading">Loading...</div>
  </DraggableWindow>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { getSystemStatus, type SystemStatus } from '@/api/client'
import DraggableWindow from './DraggableWindow.vue'

defineProps<{
  visible: boolean
}>()

defineEmits<{
  close: []
}>()

const status = ref<SystemStatus | null>(null)
let intervalId: number | null = null

async function fetchStatus() {
  try {
    status.value = await getSystemStatus()
  } catch (err) {
    console.error('Failed to fetch system status:', err)
  }
}

onMounted(() => {
  fetchStatus()
  // Update every 10 seconds
  intervalId = window.setInterval(fetchStatus, 10000)
})

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
