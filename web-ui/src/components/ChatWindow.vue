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
      <div class="messages-container" ref="messagesContainer" @scroll="handleScroll">
        <div v-if="hasMoreOlderMessages" class="load-more-container">
          <button
            v-if="!isLoadingOlderMessages"
            @click="loadMore"
            class="btn-load-more"
          >
            Load older messages
          </button>
          <div v-else class="loading-older">Loading older messages...</div>
        </div>
        <TransitionGroup name="message" tag="div" class="messages">
          <template v-for="(msg, index) in activeMessages" :key="msg.id">
            <div
              v-if="shouldShowDateSeparator(index)"
              :key="`date-${msg.id}`"
              class="date-separator"
            >
              <span class="date-separator-text">{{ formatDateSeparator(msg.timestamp) }}</span>
            </div>
            <div
              class="message"
              :class="{ sent: msg.sender === myPeerId, received: msg.sender !== myPeerId }"
            >
              <div class="message-content">
                {{ msg.content }}
              </div>
              <div class="message-time">
                {{ formatMessageTime(msg.timestamp) }}
                <span v-if="msg.sender === myPeerId" class="delivery-status">
                  {{ getDeliveryIcon(msg.delivery_status) }}
                </span>
              </div>
            </div>
          </template>
        </TransitionGroup>
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
import type { DeliveryStatus } from '@/api/types'

const conversationsStore = useConversationsStore()
const identityStore = useIdentityStore()
const {
  activeConversation,
  activeMessages,
  conversations,
  isLoadingOlderMessages,
  hasMoreOlderMessages,
} = storeToRefs(conversationsStore)
const { identity } = storeToRefs(identityStore)

const messageText = ref('')
const sending = ref(false)
const messagesContainer = ref<HTMLElement | null>(null)
const shouldAutoScroll = ref(true)
const isUserScrolling = ref(false)

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

function isSameDay(timestamp1: number, timestamp2: number): boolean {
  const d1 = new Date(timestamp1)
  const d2 = new Date(timestamp2)
  return d1.getFullYear() === d2.getFullYear() &&
         d1.getMonth() === d2.getMonth() &&
         d1.getDate() === d2.getDate()
}

function shouldShowDateSeparator(index: number): boolean {
  if (index === 0) return true // Always show for first message
  const currentMsg = activeMessages.value[index]
  const previousMsg = activeMessages.value[index - 1]
  if (!currentMsg || !previousMsg) return false
  return !isSameDay(currentMsg.timestamp, previousMsg.timestamp)
}

function formatDateSeparator(timestamp: number): string {
  const date = new Date(timestamp)
  const today = new Date()
  const yesterday = new Date(today)
  yesterday.setDate(yesterday.getDate() - 1)

  if (isSameDay(timestamp, today.getTime())) {
    return 'Today'
  } else if (isSameDay(timestamp, yesterday.getTime())) {
    return 'Yesterday'
  } else if (date.getFullYear() === today.getFullYear()) {
    return date.toLocaleDateString('en-US', { month: 'long', day: 'numeric' })
  } else {
    return date.toLocaleDateString('en-US', { year: 'numeric', month: 'long', day: 'numeric' })
  }
}

function getDeliveryIcon(status: DeliveryStatus): string {
  switch (status) {
    case 'Sending':
      return '🕐' // Clock
    case 'Sent':
      return '✓'  // Single checkmark
    case 'Delivered':
      return '✓✓' // Double checkmark
    case 'Read':
      return '✓✓' // Double checkmark (blue in CSS)
    default:
      return ''
  }
}

function checkScrollPosition() {
  if (!messagesContainer.value) return

  const container = messagesContainer.value
  const distanceFromBottom = container.scrollHeight - container.scrollTop - container.clientHeight

  // Consider "at bottom" if within 100px
  shouldAutoScroll.value = distanceFromBottom < 100
}

function handleScroll() {
  checkScrollPosition()

  // Check if user scrolled near the top - trigger load more
  if (messagesContainer.value && messagesContainer.value.scrollTop < 100) {
    if (hasMoreOlderMessages.value && !isLoadingOlderMessages.value) {
      loadMore()
    }
  }
}

async function loadMore() {
  if (!activeConversation.value || isLoadingOlderMessages.value) return

  const container = messagesContainer.value
  if (!container) return

  // Save scroll position before loading
  const oldScrollHeight = container.scrollHeight
  const oldScrollTop = container.scrollTop

  try {
    await conversationsStore.loadOlderMessages(activeConversation.value)

    // Restore scroll position after loading older messages
    await nextTick()
    const newScrollHeight = container.scrollHeight
    const heightDifference = newScrollHeight - oldScrollHeight
    container.scrollTop = oldScrollTop + heightDifference
  } catch (e) {
    console.error('Failed to load older messages:', e)
  }
}

function scrollToBottom(smooth = false) {
  if (!shouldAutoScroll.value) return

  nextTick(() => {
    if (messagesContainer.value) {
      messagesContainer.value.scrollTo({
        top: messagesContainer.value.scrollHeight,
        behavior: smooth ? 'smooth' : 'auto'
      })
    }
  })
}

async function handleSend() {
  if (!messageText.value.trim() || !activeConversation.value || sending.value) return

  const content = messageText.value.trim()
  messageText.value = ''
  sending.value = true

  try {
    await conversationsStore.sendMessage(activeConversation.value, content)
    // Scroll to bottom smoothly after sending
    scrollToBottom(true)
  } catch (e) {
    console.error('Failed to send message:', e)
    messageText.value = content // Restore message on error
  } finally {
    sending.value = false
  }
}

// Watch for new messages and scroll to bottom if user is near bottom
watch(activeMessages, (newMessages, oldMessages) => {
  // Only auto-scroll if a new message was added at the end
  if (newMessages.length > (oldMessages?.length || 0)) {
    const wasAtBottom = shouldAutoScroll.value
    if (wasAtBottom) {
      scrollToBottom(true)
    }
  }
}, { deep: false })

// Load messages when conversation changes
watch(activeConversation, async (peerId) => {
  if (peerId) {
    // Reset scroll state
    shouldAutoScroll.value = true

    await conversationsStore.fetchMessages(peerId)

    // Scroll to bottom immediately for new conversation
    nextTick(() => {
      if (messagesContainer.value) {
        messagesContainer.value.scrollTop = messagesContainer.value.scrollHeight
      }
    })
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
  scroll-behavior: smooth;
}

.load-more-container {
  display: flex;
  justify-content: center;
  padding: 8px 0 16px 0;
}

.btn-load-more {
  padding: 8px 16px;
  border: 1px solid #007bff;
  border-radius: 16px;
  background: white;
  color: #007bff;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-load-more:hover {
  background: #007bff;
  color: white;
}

.loading-older {
  text-align: center;
  color: #6c757d;
  font-size: 13px;
  padding: 8px;
}

.messages {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.date-separator {
  display: flex;
  align-items: center;
  justify-content: center;
  margin: 16px 0;
  position: relative;
}

.date-separator::before,
.date-separator::after {
  content: '';
  flex: 1;
  height: 1px;
  background: #e0e0e0;
}

.date-separator::before {
  margin-right: 12px;
}

.date-separator::after {
  margin-left: 12px;
}

.date-separator-text {
  font-size: 12px;
  color: #6c757d;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.message {
  display: flex;
  flex-direction: column;
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
  display: flex;
  align-items: center;
  gap: 4px;
}

.delivery-status {
  font-size: 10px;
  color: #6c757d;
}

/* Vue TransitionGroup animations */
.message-enter-active {
  transition: all 0.3s ease-out;
}

.message-enter-from {
  opacity: 0;
  transform: translateY(20px);
}

.message-leave-active {
  transition: all 0.2s ease-in;
}

.message-leave-to {
  opacity: 0;
  transform: translateY(-10px);
}

.message-move {
  transition: transform 0.3s ease;
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
