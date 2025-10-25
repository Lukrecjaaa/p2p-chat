<template>
  <div class="chat-window" :style="conversationThemeStyle">
    <div v-if="!conversation" class="empty-state">
      <div class="empty-icon">
        <img src="/conversation-select.ico" alt="" />
      </div>
      <h3>Select a conversation</h3>
      <p>Choose a chat from the list to start messaging</p>
    </div>
    <template v-else>
      <div class="chat-header title-bar">
        <div class="peer-info">
          <FramedAvatar
            :name="conversation.nickname || conversation.peer_id"
            :peer-id="conversation.peer_id"
            size="small"
          />
          <div>
            <div class="title-bar-text">{{ conversation.nickname || truncatePeerId(conversation.peer_id) }}</div>
            <span class="status" :class="{ online: conversation.online }">
              {{ conversation.online ? 'Online' : 'Offline' }}
            </span>
          </div>
        </div>
      </div>
      <div class="messages-container has-scrollbar" ref="messagesContainer" @scroll="handleScroll">
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
              :data-message-id="msg.id"
            >
              <div
                role="tooltip"
                class="message-tooltip"
                :class="[
                  msg.sender === myPeerId ? 'is-bottom is-left' : 'is-bottom is-right',
                  { tinted: msg.sender === myPeerId }
                ]"
              >
                <div class="message-content">{{ msg.content }}</div>
                <div class="message-meta">
                  {{ formatMessageTime(msg.timestamp) }}
                  <span v-if="msg.sender === myPeerId">
                    · <span class="mdi" :class="getDeliveryIconClass(msg.delivery_status)"></span>
                  </span>
                </div>
              </div>
            </div>
          </template>
        </TransitionGroup>
      </div>
      <div class="message-input-container">
        <form @submit.prevent="handleSend" class="field-row">
          <input
            v-model="messageText"
            type="text"
            placeholder="Type a message..."
            :disabled="sending"
          />
          <button type="submit" :disabled="!messageText.trim() || sending" class="send-button">
            <img src="/send-button.ico" alt="" class="send-icon" />
            Send
          </button>
        </form>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onUnmounted } from 'vue'
import { storeToRefs } from 'pinia'
import { useConversationsStore } from '@/stores/conversations'
import { useIdentityStore } from '@/stores/identity'
import { markMessageRead } from '@/api/client'
import type { DeliveryStatus } from '@/api/types'
import FramedAvatar from './FramedAvatar.vue'
import { getPeerBranding, ensureReadableGradient } from '@/peerBranding'

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
const readReceiptsSent = ref<Set<string>>(new Set())

const myPeerId = computed(() => identity.value?.peer_id)

// Intersection Observer for read receipts
let observer: IntersectionObserver | null = null

function setupIntersectionObserver() {
  if (observer) {
    observer.disconnect()
  }

  observer = new IntersectionObserver(
    (entries) => {
      entries.forEach((entry) => {
        if (entry.isIntersecting && entry.intersectionRatio >= 0.5) {
          const msgId = entry.target.getAttribute('data-message-id')
          if (!msgId) return

          const msg = activeMessages.value.find((m) => m.id === msgId)
          if (!msg) return

          // Only send read receipt for received messages (not sent by me)
          if (
            msg.sender !== myPeerId.value &&
            msg.delivery_status !== 'Read' &&
            !readReceiptsSent.value.has(msgId)
          ) {
            console.log('[ReadReceipt] Sending for message:', msgId)
            readReceiptsSent.value.add(msgId)
            markMessageRead(msgId).catch((err) => {
              console.error('[ReadReceipt] Failed:', err)
              readReceiptsSent.value.delete(msgId) // Retry later
            })
          }
        }
      })
    },
    { threshold: 0.5 } // 50% visible
  )

  // Observe all message elements
  nextTick(() => {
    if (!messagesContainer.value) return
    const messageElements = messagesContainer.value.querySelectorAll('[data-message-id]')
    messageElements.forEach((el) => {
      if (observer) {
        observer.observe(el)
      }
    })
  })
}

const conversation = computed(() => {
  if (!activeConversation.value) return null
  return conversations.value.find(c => c.peer_id === activeConversation.value)
})

const conversationBranding = computed(() => {
  if (!conversation.value) return null
  return getPeerBranding(conversation.value.peer_id)
})

const bubbleGradient = computed(() => {
  const gradient = conversationBranding.value?.gradient
  return ensureReadableGradient(gradient)
})

const conversationThemeStyle = computed(() => {
  const gradient = bubbleGradient.value || null
  const start = gradient ? gradient[0] : '#fafafa'
  const end = gradient ? gradient[1] : '#ececec'
  const border = gradient ? gradient[0] : '#d7dce1'
  return {
    '--conversation-bubble-start': start,
    '--conversation-bubble-end': end,
    '--conversation-border-color': border,
  }
})

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

function getDeliveryIconClass(status: DeliveryStatus): string {
  switch (status) {
    case 'Sending':
      return 'mdi-clock-outline'
    case 'Sent':
      return 'mdi-check'
    case 'Delivered':
      return 'mdi-check-all'
    case 'Read':
      return 'mdi-check-all read'
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

  // Re-setup observer for new messages
  setupIntersectionObserver()
}, { deep: false })

// Load messages when conversation changes
watch(activeConversation, async (peerId) => {
  if (peerId) {
    // Reset scroll state and read receipts
    shouldAutoScroll.value = true
    readReceiptsSent.value.clear()

    await conversationsStore.fetchMessages(peerId)

    // Scroll to bottom immediately for new conversation
    nextTick(() => {
      if (messagesContainer.value) {
        messagesContainer.value.scrollTop = messagesContainer.value.scrollHeight
      }
      // Setup observer for new conversation
      setupIntersectionObserver()
    })
  }
}, { immediate: true })

onMounted(() => {
  setupIntersectionObserver()
})

onUnmounted(() => {
  if (observer) {
    observer.disconnect()
    observer = null
  }
})
</script>

<style scoped>
.chat-window {
  flex: 1;
  display: flex;
  flex-direction: column;
  background: #f0f0f0;
}

.empty-state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  color: var(--aero-lavender-dark);
}

.empty-icon {
  margin-bottom: 16px;
}

.empty-icon img {
  width: 64px;
  height: 64px;
  image-rendering: crisp-edges;
}

.empty-state h3 {
  margin: 0 0 8px 0;
  font-size: 20px;
  color: #333;
}

.empty-state p {
  margin: 0;
  font-size: 14px;
  color: #6c757d;
}

.chat-header {
  padding: 8px 16px;
}

.peer-info {
  display: flex;
  gap: 12px;
  align-items: center;
}

.peer-info .title-bar-text {
  margin: 0 0 2px 0;
  font-size: 14px;
  font-weight: 600;
}

.status {
  font-size: 11px;
  color: #6c757d;
}

.status.online {
  color: #28a745;
}

.messages-container {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
  background: white;
}

.load-more-container {
  display: flex;
  justify-content: center;
  padding: 8px 0 16px 0;
}

.btn-load-more {
  padding: 6px 16px;
  font-size: 12px;
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
  background: #ccc;
}

.date-separator::before {
  margin-right: 12px;
}

.date-separator::after {
  margin-left: 12px;
}

.date-separator-text {
  font-size: 11px;
  color: #6c757d;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  background: white;
  padding: 4px 12px;
  border-radius: 12px;
}

.message {
  display: flex;
  flex-direction: column;
  margin-bottom: 4px;
}

.message.sent {
  align-self: flex-end;
  align-items: flex-end;
}

.message.received {
  align-self: flex-start;
  align-items: flex-start;
}

.message-tooltip {
  max-width: 500px;
  position: relative;
  display: inline-block;
  padding: 8px 12px;
  background: linear-gradient(180deg, #ffffff 0%, #f7f8fa 100%) !important;
  border: 1px solid var(--conversation-border-color, #d7dce1);
  border-radius: 8px;
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.15);
  overflow: hidden;
}

.message-tooltip::before {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background: url('/overlay.png');
  background-size: 100% 100%;
  background-repeat: no-repeat;
  pointer-events: none;
  z-index: 1;
  opacity: 0.2;
  border-radius: 8px;
}

.message-tooltip::after {
  display: none !important;
}

.message-tooltip.tinted {
  background: linear-gradient(
    135deg,
    var(--conversation-bubble-end, #ececec) 0%,
    var(--conversation-bubble-start, #fafafa) 100%
  ) !important;
  border-color: var(--conversation-border-color, #c5ccd4);
}

.message-tooltip.tinted::before {
  opacity: 0.8;
}

.message-content {
  word-wrap: break-word;
  margin-bottom: 2px;
  color: #000;
  font-size: 13px;
  position: relative;
  z-index: 2;
}

.message-meta {
  font-size: 10px;
  color: #888;
  display: flex;
  align-items: center;
  gap: 4px;
  position: relative;
  z-index: 2;
}

.message-meta .mdi {
  font-size: 11px;
}

.message-meta .mdi.mdi-check-all.read {
  color: #007bff;
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
  background: white;
  border-top: 1px solid #ccc;
}

.message-input-container .field-row {
  display: flex;
  gap: 8px;
  margin: 0;
}

.message-input-container input {
  flex: 1;
}

.send-button {
  display: flex;
  align-items: center;
  gap: 4px;
}

.send-icon {
  width: 16px;
  height: 16px;
  image-rendering: crisp-edges;
}
</style>
