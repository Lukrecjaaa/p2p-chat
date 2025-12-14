/**
 * @file ChatWindow.vue
 * @brief This component displays the chat interface for a selected conversation.
 * It includes message display, a message input field, peer information, and handles
 * message sending, loading older messages, and read receipts. It also dynamically
 * themes message bubbles based on peer branding.
 */
<template>
  <!--
    @component ChatWindow
    @description Displays the chat interface for a selected conversation,
    including messages, message input, and peer information.
    Handles message sending, loading older messages, and read receipts.
  -->
  <div class="chat-window" :style="conversationThemeStyle">
    <!-- @section Empty State -->
    <div v-if="!conversation" class="empty-state">
      <div class="empty-icon">
        <img src="/conversation-select.png" alt="" />
      </div>
      <h3>Select a conversation</h3>
      <p>Choose a chat from the list to start messaging</p>
    </div>
    <!-- @section Chat Content (if conversation is selected) -->
    <template v-else>
      <!-- @element chat-header - Displays the active conversation's peer information. -->
      <div class="chat-header title-bar">
        <div class="peer-info">
          <!--
            @component FramedAvatar
            @description Displays the peer's avatar and name.
            @prop {string} name - The name or peer ID to display.
            @prop {string} peerId - The peer ID for avatar generation.
            @prop {string} size - The size of the avatar ('small').
          -->
          <FramedAvatar
            :name="conversation.nickname || conversation.peer_id"
            :peer-id="conversation.peer_id"
            size="small"
          />
          <div>
            <!-- @element peer-name - Displays the nickname or truncated peer ID. -->
            <div class="title-bar-text">{{ conversation.nickname || truncatePeerId(conversation.peer_id) }}</div>
            <!-- @element peer-status - Displays the online/offline status. -->
            <span class="status" :class="{ online: conversation.online }">
              {{ conversation.online ? 'Online' : 'Offline' }}
            </span>
          </div>
        </div>
      </div>
      <!-- @element messages-container - Scrollable container for chat messages. -->
      <div class="messages-container has-scrollbar" ref="messagesContainer" @scroll="handleScroll">
        <!-- @element load-more-container - Area for loading older messages button/indicator. -->
        <div v-if="hasMoreOlderMessages" class="load-more-container">
          <!-- @element btn-load-more - Button to load more older messages. -->
          <button
            v-if="!isLoadingOlderMessages"
            @click="loadMore"
            class="btn-load-more"
          >
            Load older messages
          </button>
          <!-- @element loading-older - Text indicating older messages are being loaded. -->
          <div v-else class="loading-older">Loading older messages...</div>
        </div>
        <!-- @element messages - Container for individual message elements. Uses TransitionGroup for animations. -->
        <TransitionGroup name="message" tag="div" class="messages">
          <template v-for="(msg, index) in activeMessages" :key="msg.id">
            <!-- @element date-separator - Separator displaying the date for message groups. -->
            <div
              v-if="shouldShowDateSeparator(index)"
              :key="`date-${msg.id}`"
              class="date-separator"
            >
              <span class="date-separator-text">{{ formatDateSeparator(msg.timestamp) }}</span>
            </div>
            <!-- @element message - Individual message bubble. -->
            <div
              class="message"
              :class="{ sent: msg.sender === myPeerId, received: msg.sender !== myPeerId }"
              :data-message-id="msg.id"
            >
              <!-- @element message-tooltip - The actual message content and metadata wrapper. -->
              <div
                role="tooltip"
                class="message-tooltip"
                :class="[
                  msg.sender === myPeerId ? 'is-bottom is-left' : 'is-bottom is-right',
                  { tinted: msg.sender === myPeerId }
                ]"
              >
                <!-- @element message-content - The textual content of the message. -->
                <div class="message-content">{{ msg.content }}</div>
                <!-- @element message-meta - Contains message timestamp and delivery status icon. -->
                <div class="message-meta">
                  {{ formatMessageTime(msg.timestamp) }}
                  <span v-if="msg.sender === myPeerId">
                    <!-- @element delivery-icon - Icon indicating message delivery status. -->
                    · <span class="mdi" :class="getDeliveryIconClass(msg.delivery_status)"></span>
                  </span>
                </div>
              </div>
            </div>
          </template>
        </TransitionGroup>
      </div>
      <!-- @element message-input-container - Area for message input field and send button. -->
      <div class="message-input-container">
        <!-- @element message-form - Form for typing and sending messages. -->
        <form @submit.prevent="handleSend" class="field-row">
          <!-- @element message-input - Input field for typing messages. -->
          <input
            ref="messageInput"
            v-model="messageText"
            type="text"
            placeholder="Type a message..."
            :disabled="sending"
          />
          <!-- @element send-button - Button to send the typed message. -->
          <button type="submit" :disabled="!messageText.trim() || sending" class="send-button">
            <img src="/send-button.png" alt="" class="send-icon" />
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

/**
 * Pinia store for managing conversations state and actions.
 * @type {ReturnType<typeof useConversationsStore>}
 */
const conversationsStore = useConversationsStore()
/**
 * Pinia store for managing the user's identity.
 * @type {ReturnType<typeof useIdentityStore>}
 */
const identityStore = useIdentityStore()
/**
 * Reactive references to conversation-related state from the conversations store.
 * @property {Ref<string | null>} activeConversation - The peer ID of the currently active conversation.
 * @property {Ref<Array>} activeMessages - List of messages for the active conversation.
 * @property {Ref<Array>} conversations - All known conversations.
 * @property {Ref<boolean>} isLoadingOlderMessages - Flag indicating if older messages are being loaded.
 * @property {Ref<boolean>} hasMoreOlderMessages - Flag indicating if there are more older messages to load.
 */
const {
  activeConversation,
  activeMessages,
  conversations,
  isLoadingOlderMessages,
  hasMoreOlderMessages,
} = storeToRefs(conversationsStore)
/**
 * Reactive reference to the user's identity from the identity store.
 * @property {Ref<Identity | null>} identity - The current user's identity object.
 */
const { identity } = storeToRefs(identityStore)

/**
 * Reactive state for the message input field text.
 * @type {Ref<string>}
 */
const messageText = ref('')
/**
 * Reactive state indicating if a message is currently being sent.
 * @type {Ref<boolean>}
 */
const sending = ref(false)
/**
 * Template ref for the messages container DOM element.
 * @type {Ref<HTMLElement | null>}
 */
const messagesContainer = ref<HTMLElement | null>(null)
/**
 * Template ref for the message input field DOM element.
 * @type {Ref<HTMLInputElement | null>}
 */
const messageInput = ref<HTMLInputElement | null>(null)
/**
 * Reactive state to control automatic scrolling to the bottom of the chat.
 * @type {Ref<boolean>}
 */
const shouldAutoScroll = ref(true)
/**
 * Reactive state to track if the user is actively scrolling. (Currently not used but can be for more complex logic)
 * @type {Ref<boolean>}
 */
const isUserScrolling = ref(false) // Not currently used, but useful for more complex scroll logic
/**
 * Set to store IDs of messages for which read receipts have already been sent.
 * Prevents duplicate read receipt API calls.
 * @type {Ref<Set<string>>}
 */
const readReceiptsSent = ref<Set<string>>(new Set())

/**
 * Computed property that returns the current user's peer ID.
 * @computed
 * @returns {string | undefined} The peer ID or undefined if not available.
 */
const myPeerId = computed(() => identity.value?.peer_id)

/**
 * Intersection Observer instance for detecting message visibility for read receipts.
 * @type {IntersectionObserver | null}
 */
let observer: IntersectionObserver | null = null

/**
 * Sets up the Intersection Observer to monitor message elements for visibility.
 * When a received message becomes 50% visible, a read receipt is sent.
 * Disconnects any existing observer before creating a new one.
 * @function setupIntersectionObserver
 */
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

          // Only send read receipt for received messages (not sent by me) and if not already read/sent
          if (
            msg.sender !== myPeerId.value &&
            msg.delivery_status !== 'Read' &&
            !readReceiptsSent.value.has(msgId)
          ) {
            console.log('[ReadReceipt] Sending for message:', msgId)
            readReceiptsSent.value.add(msgId)
            markMessageRead(msgId).catch((err) => {
              console.error('[ReadReceipt] Failed:', err)
              readReceiptsSent.value.delete(msgId) // Allow retry later if API call failed
            })
          }
        }
      })
    },
    { threshold: 0.5 } // Trigger when 50% of the target is visible
  )

  // Observe all message elements after the next DOM update
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

/**
 * Computed property that returns the currently active conversation object.
 * @computed
 * @returns {Conversation | null} The active conversation object or null if none is active.
 */
const conversation = computed(() => {
  if (!activeConversation.value) return null
  return conversations.value.find(c => c.peer_id === activeConversation.value)
})

/**
 * Computed property that retrieves branding information for the active conversation's peer.
 * @computed
 * @returns {object | null} Branding object for the peer, including gradient, or null.
 */
const conversationBranding = computed(() => {
  if (!conversation.value) return null
  return getPeerBranding(conversation.value.peer_id)
})

/**
 * Computed property that ensures the gradient for message bubbles is readable.
 * @computed
 * @returns {Array<string> | null} An array of two color strings representing the gradient, or null.
 */
const bubbleGradient = computed(() => {
  const gradient = conversationBranding.value?.gradient
  return ensureReadableGradient(gradient)
})

/**
 * Computed property that generates CSS style object for conversation-specific theming.
 * Uses custom CSS properties for message bubble colors.
 * @computed
 * @returns {object} CSS style object.
 */
const conversationThemeStyle = computed(() => {
  const gradient = bubbleGradient.value || null
  const start = gradient ? gradient[0] : '#fafafa' // Default start color
  const end = gradient ? gradient[1] : '#ececec'   // Default end color
  const border = gradient ? gradient[0] : '#d7dce1' // Default border color
  return {
    '--conversation-bubble-start': start,
    '--conversation-bubble-end': end,
    '--conversation-border-color': border,
  }
})

/**
 * Truncates a peer ID for display, showing the beginning and end with an ellipsis.
 * @function truncatePeerId
 * @param {string} peerId - The full peer ID.
 * @returns {string} The truncated peer ID.
 */
function truncatePeerId(peerId: string): string {
  if (peerId.length <= 12) return peerId
  return peerId.substring(0, 8) + '...' + peerId.substring(peerId.length - 4)
}

/**
 * Formats a message timestamp into a local time string (e.g., "HH:MM AM/PM").
 * @function formatMessageTime
 * @param {number} timestamp - The Unix timestamp in milliseconds.
 * @returns {string} The formatted time string.
 */
function formatMessageTime(timestamp: number): string {
  const date = new Date(timestamp)
  return date.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' })
}

/**
 * Checks if two timestamps fall on the same calendar day.
 * @function isSameDay
 * @param {number} timestamp1 - The first Unix timestamp.
 * @param {number} timestamp2 - The second Unix timestamp.
 * @returns {boolean} True if both timestamps are on the same day, false otherwise.
 */
function isSameDay(timestamp1: number, timestamp2: number): boolean {
  const d1 = new Date(timestamp1)
  const d2 = new Date(timestamp2)
  return d1.getFullYear() === d2.getFullYear() &&
         d1.getMonth() === d2.getMonth() &&
         d1.getDate() === d2.getDate()
}

/**
 * Determines if a date separator should be displayed before a message.
 * A separator is shown for the first message or if the current message's date differs from the previous one.
 * @function shouldShowDateSeparator
 * @param {number} index - The index of the current message in the activeMessages array.
 * @returns {boolean} True if a date separator should be shown, false otherwise.
 */
function shouldShowDateSeparator(index: number): boolean {
  if (index === 0) return true // Always show for the very first message
  const currentMsg = activeMessages.value[index]
  const previousMsg = activeMessages.value[index - 1]
  if (!currentMsg || !previousMsg) return false // Should not happen if index > 0
  return !isSameDay(currentMsg.timestamp, previousMsg.timestamp)
}

/**
 * Formats a timestamp to be displayed as a date separator (e.g., "Today", "Yesterday", "Month Day, Year").
 * @function formatDateSeparator
 * @param {number} timestamp - The Unix timestamp in milliseconds.
 * @returns {string} The formatted date string.
 */
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

/**
 * Returns the appropriate Material Design Icon class based on the message delivery status.
 * @function getDeliveryIconClass
 * @param {DeliveryStatus} status - The delivery status of the message.
 * @returns {string} The CSS class for the delivery icon.
 */
function getDeliveryIconClass(status: DeliveryStatus): string {
  switch (status) {
    case 'Sending':
      return 'mdi-clock-outline' // Icon for message being sent
    case 'Sent':
      return 'mdi-check'         // Icon for message sent to server
    case 'Delivered':
      return 'mdi-check-all'     // Icon for message delivered to recipient
    case 'Read':
      return 'mdi-check-all read' // Icon for message read by recipient
    default:
      return ''
  }
}

/**
 * Checks the current scroll position of the messages container to determine
 * if auto-scrolling should be enabled. Auto-scrolls if the user is near the bottom.
 * @function checkScrollPosition
 */
function checkScrollPosition() {
  if (!messagesContainer.value) return

  const container = messagesContainer.value
  // Distance from the bottom of the scrollable area
  const distanceFromBottom = container.scrollHeight - container.scrollTop - container.clientHeight

  // If the user is within 100 pixels from the bottom, enable auto-scroll
  shouldAutoScroll.value = distanceFromBottom < 100
}

/**
 * Handles scroll events on the messages container.
 * Calls `checkScrollPosition` and triggers `loadMore` if scrolled near the top.
 * @function handleScroll
 */
function handleScroll() {
  checkScrollPosition()

  // Check if user scrolled near the top - trigger load more
  if (messagesContainer.value && messagesContainer.value.scrollTop < 100) {
    // Only load more if there are older messages and not already loading
    if (hasMoreOlderMessages.value && !isLoadingOlderMessages.value) {
      loadMore()
    }
  }
}

/**
 * Loads older messages for the active conversation.
 * Preserves scroll position after new messages are loaded.
 * @async
 * @function loadMore
 * @returns {Promise<void>}
 */
async function loadMore() {
  if (!activeConversation.value || isLoadingOlderMessages.value) return

  const container = messagesContainer.value
  if (!container) return

  // Save scroll position before loading new content
  const oldScrollHeight = container.scrollHeight
  const oldScrollTop = container.scrollTop

  try {
    // Dispatch action to load older messages
    await conversationsStore.loadOlderMessages(activeConversation.value)

    // Wait for DOM to update with new messages, then restore scroll position
    await nextTick()
    const newScrollHeight = container.scrollHeight
    const heightDifference = newScrollHeight - oldScrollHeight
    container.scrollTop = oldScrollTop + heightDifference
  } catch (e) {
    console.error('Failed to load older messages:', e)
  }
}

/**
 * Scrolls the messages container to the bottom.
 * @function scrollToBottom
 * @param {boolean} [smooth=false] - Whether to use smooth scrolling behavior.
 */
function scrollToBottom(smooth = false) {
  // Only auto-scroll if `shouldAutoScroll` is true
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

/**
 * Handles sending a new message.
 * Clears the input, sets sending state, dispatches message to store, scrolls to bottom,
 * and refocuses the input field for quick follow-up messages.
 * @async
 * @function handleSend
 * @returns {Promise<void>}
 */
async function handleSend() {
  if (!messageText.value.trim() || !activeConversation.value || sending.value) return

  const content = messageText.value.trim()
  messageText.value = '' // Clear input immediately
  sending.value = true // Set sending state to prevent duplicate sends

  try {
    await conversationsStore.sendMessage(activeConversation.value, content)
    // Scroll to bottom smoothly after successful send
    scrollToBottom(true)
  } catch (e) {
    console.error('Failed to send message:', e)
    messageText.value = content // Restore message if sending failed
  } finally {
    sending.value = false // Reset sending state
    // Refocus the input field to allow quick follow-up messages
    nextTick(() => {
      messageInput.value?.focus()
    })
  }
}

/**
 * Watcher for `activeMessages`.
 * Auto-scrolls to the bottom if new messages are added and `shouldAutoScroll` is true.
 * Re-sets up the Intersection Observer when messages change to monitor new elements.
 * @function watch
 * @param {Array} newMessages - The new array of messages.
 * @param {Array | undefined} oldMessages - The previous array of messages.
 */
watch(activeMessages, (newMessages, oldMessages) => {
  // Only auto-scroll if a new message was added at the end (increased length)
  if (newMessages.length > (oldMessages?.length || 0)) {
    const wasAtBottom = shouldAutoScroll.value
    if (wasAtBottom) {
      scrollToBottom(true)
    }
  }

  // Re-setup observer for any new message elements that might have been added
  setupIntersectionObserver()
}, { deep: false }) // Deep watch is not necessary for array length check and initial setup

/**
 * Watcher for `activeConversation`.
 * When the active conversation changes, it resets scroll state, clears read receipts,
 * fetches messages for the new conversation, and scrolls to the bottom.
 * @function watch
 * @param {string | null} peerId - The peer ID of the newly active conversation.
 * @fires conversationsStore.fetchMessages
 * @fires setupIntersectionObserver
 */
watch(activeConversation, async (peerId) => {
  if (peerId) {
    // Reset scroll state and read receipts for the new conversation
    shouldAutoScroll.value = true
    readReceiptsSent.value.clear()

    // Fetch messages for the newly active conversation
    await conversationsStore.fetchMessages(peerId)

    // After messages are fetched and rendered, scroll to bottom
    nextTick(() => {
      if (messagesContainer.value) {
        messagesContainer.value.scrollTop = messagesContainer.value.scrollHeight
      }
      // Setup observer for the messages in the new conversation
      setupIntersectionObserver()
    })
  }
}, { immediate: true }) // Run immediately on component mount if activeConversation already has a value

/**
 * Lifecycle hook: Called after the component has mounted.
 * Sets up the Intersection Observer to monitor message visibility.
 * @function onMounted
 */
onMounted(() => {
  setupIntersectionObserver()
})

/**
 * Lifecycle hook: Called before the component unmounts.
 * Disconnects the Intersection Observer to prevent memory leaks.
 * @function onUnmounted
 */
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

