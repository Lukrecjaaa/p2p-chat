<template>
  <div class="status-panel">
    <h3 class="status-title">System Status</h3>
    <div v-if="status" class="status-content">
      <div class="status-item">
        <div class="status-icon">🌐</div>
        <div class="status-info">
          <div class="status-label">Connected Peers</div>
          <div class="status-value">{{ status.connected_peers }}</div>
        </div>
      </div>
      <div class="status-item">
        <div class="status-icon">📬</div>
        <div class="status-info">
          <div class="status-label">Known Mailboxes</div>
          <div class="status-value">{{ status.known_mailboxes }}</div>
        </div>
      </div>
      <div class="status-item">
        <div class="status-icon">⏳</div>
        <div class="status-info">
          <div class="status-label">Pending Messages</div>
          <div class="status-value">{{ status.pending_messages }}</div>
        </div>
      </div>
    </div>
    <div v-else class="status-loading">Loading...</div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { getSystemStatus, type SystemStatus } from '@/api/client'

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
.status-panel {
  padding: 16px;
  border-bottom: 1px solid #e0e0e0;
  background: #f8f9fa;
}

.status-title {
  margin: 0 0 12px 0;
  font-size: 14px;
  font-weight: 600;
  color: #212529;
}

.status-content {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.status-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px;
  background: #fff;
  border-radius: 6px;
  border: 1px solid #e0e0e0;
}

.status-icon {
  font-size: 20px;
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.status-info {
  flex: 1;
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.status-label {
  font-size: 12px;
  color: #6c757d;
  font-weight: 500;
}

.status-value {
  font-size: 14px;
  font-weight: 700;
  color: #007bff;
}

.status-loading {
  font-size: 12px;
  color: #6c757d;
  text-align: center;
  padding: 8px;
}
</style>
