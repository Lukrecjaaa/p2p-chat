<template>
  <div class="chat-list">
    <div class="chat-list-header">
      <h2>Chats</h2>
      <button @click="$emit('openAddFriend')" class="btn-add">+</button>
    </div>
    <div v-if="loading" class="loading">Loading...</div>
    <div v-else-if="error" class="error">{{ error }}</div>
    <div v-else class="conversations">
      <div
        v-for="conv in sortedConversations"
        :key="conv.peer_id"
        class="conversation-item"
        :class="{ active: conv.peer_id === activeConversation }"
        @click="$emit('selectConversation', conv.peer_id)"
      >
        <div class="avatar">
          <div class="status-dot" :class="{ online: conv.online }"></div>
          {{ getInitials(conv.nickname || conv.peer_id) }}
        </div>
        <div class="conversation-info">
          <div class="conversation-header">
            <span class="peer-name">{{ conv.nickname || truncatePeerId(conv.peer_id) }}</span>
            <span v-if="conv.last_message" class="timestamp">
              {{ formatTimestamp(conv.last_message.timestamp) }}
            </span>
          </div>
          <div class="last-message">
            {{ conv.last_message?.content || 'No messages yet' }}
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { storeToRefs } from 'pinia'
import { useConversationsStore } from '@/stores/conversations'

defineEmits<{
  selectConversation: [peerId: string]
  openAddFriend: []
}>()

const conversationsStore = useConversationsStore()
const { sortedConversations, activeConversation, loading, error } = storeToRefs(conversationsStore)

function getInitials(name: string): string {
  return name.substring(0, 2).toUpperCase()
}

function truncatePeerId(peerId: string): string {
  if (peerId.length <= 12) return peerId
  return peerId.substring(0, 8) + '...' + peerId.substring(peerId.length - 4)
}

function formatTimestamp(timestamp: number): string {
  const date = new Date(timestamp)
  const now = new Date()
  const diff = now.getTime() - date.getTime()
  const hours = diff / (1000 * 60 * 60)

  if (hours < 24) {
    return date.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' })
  } else if (hours < 168) {
    return date.toLocaleDateString('en-US', { weekday: 'short' })
  } else {
    return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
  }
}
</script>

<style scoped>
.chat-list {
  width: 320px;
  border-right: 1px solid #e0e0e0;
  display: flex;
  flex-direction: column;
  background: #fff;
}

.chat-list-header {
  padding: 16px;
  border-bottom: 1px solid #e0e0e0;
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.chat-list-header h2 {
  margin: 0;
  font-size: 20px;
  font-weight: 600;
}

.btn-add {
  width: 36px;
  height: 36px;
  border-radius: 50%;
  border: none;
  background: #007bff;
  color: white;
  font-size: 24px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  line-height: 1;
}

.btn-add:hover {
  background: #0056b3;
}

.loading,
.error {
  padding: 16px;
  text-align: center;
}

.error {
  color: #dc3545;
}

.conversations {
  overflow-y: auto;
  flex: 1;
}

.conversation-item {
  display: flex;
  gap: 12px;
  padding: 12px 16px;
  cursor: pointer;
  border-bottom: 1px solid #f0f0f0;
  transition: background 0.15s;
}

.conversation-item:hover {
  background: #f8f9fa;
}

.conversation-item.active {
  background: #e3f2fd;
}

.avatar {
  position: relative;
  width: 48px;
  height: 48px;
  border-radius: 50%;
  background: #6c757d;
  color: white;
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: 600;
  flex-shrink: 0;
}

.status-dot {
  position: absolute;
  bottom: 2px;
  right: 2px;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: #6c757d;
  border: 2px solid white;
}

.status-dot.online {
  background: #28a745;
}

.conversation-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.conversation-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.peer-name {
  font-weight: 600;
  font-size: 14px;
  color: #212529;
}

.timestamp {
  font-size: 12px;
  color: #6c757d;
  flex-shrink: 0;
}

.last-message {
  font-size: 13px;
  color: #6c757d;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
