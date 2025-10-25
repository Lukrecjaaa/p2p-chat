<template>
  <div class="info-panel">
    <h3 class="info-title">Your Info</h3>
    <div v-if="identity" class="info-content">
      <div class="info-item">
        <label>Peer ID:</label>
        <div class="info-value">
          <code>{{ identity.peer_id }}</code>
          <button @click="copyToClipboard(identity.peer_id)" class="btn-copy" title="Copy">
            📋
          </button>
        </div>
      </div>
      <div class="info-item">
        <label>Public Key:</label>
        <div class="info-value">
          <code>{{ truncateKey(identity.hpke_public_key) }}</code>
          <button @click="copyToClipboard(identity.hpke_public_key)" class="btn-copy" title="Copy">
            📋
          </button>
        </div>
      </div>
    </div>
    <div v-else class="info-loading">Loading...</div>
  </div>
</template>

<script setup lang="ts">
import { storeToRefs } from 'pinia'
import { useIdentityStore } from '@/stores/identity'

const identityStore = useIdentityStore()
const { identity } = storeToRefs(identityStore)

function truncateKey(key: string): string {
  if (key.length <= 40) return key
  return key.substring(0, 20) + '...' + key.substring(key.length - 20)
}

async function copyToClipboard(text: string) {
  try {
    await navigator.clipboard.writeText(text)
  } catch (err) {
    console.error('Failed to copy:', err)
  }
}
</script>

<style scoped>
.info-panel {
  padding: 16px;
  border-bottom: 1px solid #e0e0e0;
  background: #f8f9fa;
}

.info-title {
  margin: 0 0 12px 0;
  font-size: 14px;
  font-weight: 600;
  color: #212529;
}

.info-content {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.info-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.info-item label {
  font-size: 11px;
  font-weight: 600;
  color: #6c757d;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.info-value {
  display: flex;
  align-items: center;
  gap: 8px;
}

.info-value code {
  flex: 1;
  font-size: 11px;
  padding: 6px 8px;
  background: #fff;
  border: 1px solid #e0e0e0;
  border-radius: 4px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: 'Monaco', 'Courier New', monospace;
}

.btn-copy {
  padding: 4px 8px;
  border: none;
  background: transparent;
  cursor: pointer;
  font-size: 14px;
  opacity: 0.6;
  transition: opacity 0.2s;
}

.btn-copy:hover {
  opacity: 1;
}

.info-loading {
  font-size: 12px;
  color: #6c757d;
  text-align: center;
  padding: 8px;
}
</style>
