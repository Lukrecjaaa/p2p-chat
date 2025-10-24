<template>
  <div class="chat-window">
    <div v-if="!conversation" class="empty-state">
      <div class="empty-icon">💬</div>
      <h3>Select a conversation</h3>
      <p>Choose a chat from the list to start messaging</p>
    </div>
    <template v-else>
      <div class="chat-header">
        <div class="peer-info">
          <div class="avatar">{{ getInitials(conversation.nickname || conversation.peer_id) }}</div>
          <div>
            <h3>{{ conversation.nickname || truncatePeerId(conversation.peer_id) }}</h3>
            <span class="status" :class="{ online: conversation.online }">
              {{ conversation.online ? 'Online' : 'Offline' }}
            </span>
          </div>
        </div>
      </div>
      <div class="messages-container" ref="messagesContainer">
        <div v-if="loading" class="loading">Loading messages...</div>
        <div v-else class="messages">
          <div
            v-for="msg in activeMessages"
            :key="msg.id"
            class="message"
            :class="{ sent: msg.sender === myPeerId, received: msg.sender !== myPeerId }"
          >
            <div class="message-content">
              {{ msg.content }}
            </div>
            <div class="message-time">
              {{ formatMessageTime(msg.timestamp) }}
            </div>
          </div>
        </div>
      </div>
      <div class="message-input-container">
        <form @submit.prevent="handleSend">
          <input
            v-model="messageText"
            type="text"
            placeholder="Type a message..."
            class="message-input"
            :disabled="sending"
          />
          <button type="submit" class="btn-send" :disabled="!messageText.trim() || sending">
            Send
          </button>
        </form>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick } from 'vue'
import { storeToRefs } from 'pinia'
import { useConversationsStore } from '@/stores/conversations'
import { useIdentityStore } from '@/stores/identity'

const conversationsStore = useConversationsStore()
const identityStore = useIdentityStore()
const { activeConversation, activeMessages, conversations, loading } = storeToRefs(conversationsStore)
const { identity } = storeToRefs(identityStore)

const messageText = ref('')
const sending = ref(false)
const messagesContainer = ref<HTMLElement | null>(null)

const myPeerId = computed(() => identity.value?.peer_id)

const conversation = computed(() => {
  if (!activeConversation.value) return null
  return conversations.value.find(c => c.peer_id === activeConversation.value)
})

function getInitials(name: string): string {
  return name.substring(0, 2).toUpperCase()
}

function truncatePeerId(peerId: string): string {
  if (peerId.length <= 12) return peerId
  return peerId.substring(0, 8) + '...' + peerId.substring(peerId.length - 4)
}

function formatMessageTime(timestamp: number): string {
  const date = new Date(timestamp)
  return date.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' })
}

async function handleSend() {
  if (!messageText.value.trim() || !activeConversation.value || sending.value) return

  const content = messageText.value.trim()
  messageText.value = ''
  sending.value = true

  try {
    await conversationsStore.sendMessage(activeConversation.value, content)
    scrollToBottom()
  } catch (e) {
    console.error('Failed to send message:', e)
    messageText.value = content // Restore message on error
  } finally {
    sending.value = false
  }
}

function scrollToBottom() {
  nextTick(() => {
    if (messagesContainer.value) {
      messagesContainer.value.scrollTop = messagesContainer.value.scrollHeight
    }
  })
}

// Watch for new messages and scroll to bottom
watch(activeMessages, () => {
  scrollToBottom()
}, { deep: true })

// Load messages when conversation changes
watch(activeConversation, async (peerId) => {
  if (peerId) {
    await conversationsStore.fetchMessages(peerId)
    scrollToBottom()
  }
}, { immediate: true })
</script>

<style scoped>
.chat-window {
  flex: 1;
  display: flex;
  flex-direction: column;
  background: #f8f9fa;
}

.empty-state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  color: #6c757d;
}

.empty-icon {
  font-size: 64px;
  margin-bottom: 16px;
}

.empty-state h3 {
  margin: 0 0 8px 0;
  font-size: 20px;
}

.empty-state p {
  margin: 0;
  font-size: 14px;
}

.chat-header {
  padding: 16px;
  border-bottom: 1px solid #e0e0e0;
  background: #fff;
}

.peer-info {
  display: flex;
  gap: 12px;
  align-items: center;
}

.peer-info .avatar {
  width: 40px;
  height: 40px;
  border-radius: 50%;
  background: #6c757d;
  color: white;
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: 600;
}

.peer-info h3 {
  margin: 0 0 4px 0;
  font-size: 16px;
  font-weight: 600;
}

.status {
  font-size: 13px;
  color: #6c757d;
}

.status.online {
  color: #28a745;
}

.messages-container {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
}

.loading {
  text-align: center;
  color: #6c757d;
  padding: 16px;
}

.messages {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.message {
  display: flex;
  flex-direction: column;
  max-width: 60%;
}

.message.sent {
  align-self: flex-end;
  align-items: flex-end;
}

.message.received {
  align-self: flex-start;
  align-items: flex-start;
}

.message-content {
  padding: 10px 14px;
  border-radius: 16px;
  word-wrap: break-word;
}

.message.sent .message-content {
  background: #007bff;
  color: white;
  border-bottom-right-radius: 4px;
}

.message.received .message-content {
  background: #fff;
  color: #212529;
  border-bottom-left-radius: 4px;
  border: 1px solid #e0e0e0;
}

.message-time {
  font-size: 11px;
  color: #6c757d;
  margin-top: 4px;
  padding: 0 4px;
}

.message-input-container {
  padding: 16px;
  border-top: 1px solid #e0e0e0;
  background: #fff;
}

.message-input-container form {
  display: flex;
  gap: 8px;
}

.message-input {
  flex: 1;
  padding: 10px 14px;
  border: 1px solid #e0e0e0;
  border-radius: 24px;
  font-size: 14px;
  outline: none;
}

.message-input:focus {
  border-color: #007bff;
}

.btn-send {
  padding: 10px 24px;
  border: none;
  border-radius: 24px;
  background: #007bff;
  color: white;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.15s;
}

.btn-send:hover:not(:disabled) {
  background: #0056b3;
}

.btn-send:disabled {
  background: #6c757d;
  cursor: not-allowed;
}
</style>
