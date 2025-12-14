/**
 * @file conversations.ts
 * @brief Pinia store for managing conversations and messages within the application.
 * This store handles fetching, sending, and organizing messages for different peers,
 * as well as managing the active conversation and message loading states.
 */
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { Conversation, Message } from '@/api/types'
import { listConversations, getMessages, sendMessage as apiSendMessage } from '@/api/client'
import { useIdentityStore } from './identity'

/**
 * Interface representing the structure of message data for a single peer within the store.
 * Each peer has its own MessageStore to manage their conversation history.
 * @interface MessageStore
 * @property {Map<string, Message>} messagesById - A map of messages, keyed by message ID for quick lookup.
 * @property {string[]} sortedIds - An array of message IDs, sorted by timestamp and nonce, representing the display order.
 * @property {string | null} oldestLoadedId - The ID of the oldest message currently loaded for this peer.
 * @property {string | null} newestLoadedId - The ID of the newest message currently loaded for this peer.
 * @property {boolean} hasMoreOlder - Indicates if there are more older messages to load for this conversation.
 * @property {boolean} isLoadingOlder - Indicates if older messages are currently being loaded.
 */
interface MessageStore {
  messagesById: Map<string, Message>
  sortedIds: string[]
  oldestLoadedId: string | null
  newestLoadedId: string | null
  hasMoreOlder: boolean
  isLoadingOlder: boolean
}

/**
 * Pinia store for managing conversations and messages.
 * @returns {object} The store's state, getters, and actions.
 * @property {Ref<Conversation[]>} conversations - Reactive array of all conversations.
 * @property {Ref<Map<string, MessageStore>>} messages - Map storing message data for each peer.
 * @property {Ref<string | null>} activeConversation - The peer ID of the currently active conversation.
 * @property {Ref<boolean>} loading - Indicates if conversations are currently being fetched.
 * @property {Ref<string | null>} error - Stores any error message if an operation fails.
 * @property {ComputedRef<Conversation[]>} sortedConversations - Conversations sorted by the last message timestamp.
 * @property {ComputedRef<Message[]>} activeMessages - Messages for the active conversation, sorted chronologically.
 * @property {ComputedRef<boolean>} isLoadingOlderMessages - Indicates if older messages for the active conversation are loading.
 * @property {ComputedRef<boolean>} hasMoreOlderMessages - Indicates if there are more older messages to load for the active conversation.
 * @property {Function} fetchConversations - Action to fetch the list of conversations.
 * @property {Function} fetchMessages - Action to fetch messages for a specific peer.
 * @property {Function} loadOlderMessages - Action to load older messages for the active conversation.
 * @property {Function} sendMessage - Action to send a message to a peer.
 * @property {Function} insertMessage - Helper to insert a message into the correct message store.
 * @property {Function} setActiveConversation - Sets the currently active conversation.
 * @property {Function} updatePeerOnlineStatus - Updates the online status of a peer in conversations.
 * @property {Function} updateConversationLastMessage - Updates the last message for a conversation.
 * @property {Function} updateMessageDeliveryStatus - Updates the delivery status of a specific message.
 */
export const useConversationsStore = defineStore('conversations', () => {
  /**
   * Reactive array of all conversations, each containing a peer ID, nickname, and last message.
   * @type {Ref<Conversation[]>}
   */
  const conversations = ref<Conversation[]>([])
  /**
   * A map where keys are peer IDs and values are `MessageStore` objects,
   * holding messages specific to that peer.
   * @type {Ref<Map<string, MessageStore>>}
   */
  const messages = ref<Map<string, MessageStore>>(new Map())
  /**
   * The peer ID of the currently active conversation. Null if no conversation is active.
   * @type {Ref<string | null>}
   */
  const activeConversation = ref<string | null>(null)
  /**
   * Boolean indicating if conversations data is currently being loaded.
   * @type {Ref<boolean>}
   */
  const loading = ref(false)
  /**
   * Stores an error message if an operation fails. Null if no error.
   * @type {Ref<string | null>}
   */
  const error = ref<string | null>(null)

  /**
   * Computed property that returns conversations sorted by the timestamp of their last message
   * in descending order (most recent first).
   * @type {ComputedRef<Conversation[]>}
   */
  const sortedConversations = computed(() => {
    return [...conversations.value].sort((a, b) => {
      const aTime = a.last_message?.timestamp || 0
      const bTime = b.last_message?.timestamp || 0
      return bTime - aTime
    })
  })

  /**
   * Computed property that returns messages for the currently active conversation,
   * sorted chronologically. Returns an empty array if no conversation is active.
   * @type {ComputedRef<Message[]>}
   */
  const activeMessages = computed(() => {
    if (!activeConversation.value) return []
    const store = messages.value.get(activeConversation.value)
    if (!store) return []

    return store.sortedIds.map(id => store.messagesById.get(id)!)
  })

  /**
   * Computed property that indicates whether older messages for the active conversation are currently being loaded.
   * @type {ComputedRef<boolean>}
   */
  const isLoadingOlderMessages = computed(() => {
    if (!activeConversation.value) return false
    const store = messages.value.get(activeConversation.value)
    return store?.isLoadingOlder || false
  })

  /**
   * Computed property that indicates whether there are more older messages available to load
   * for the active conversation.
   * @type {ComputedRef<boolean>}
   */
  const hasMoreOlderMessages = computed(() => {
    if (!activeConversation.value) return false
    const store = messages.value.get(activeConversation.value)
    return store?.hasMoreOlder || false
  })

  /**
   * Retrieves an existing `MessageStore` for a given peerId, or creates a new one if it doesn't exist.
   * @param {string} peerId - The ID of the peer.
   * @returns {MessageStore} The message store for the specified peer.
   */
  function getOrCreateMessageStore(peerId: string): MessageStore {
    if (!messages.value.has(peerId)) {
      messages.value.set(peerId, {
        messagesById: new Map(),
        sortedIds: [],
        oldestLoadedId: null,
        newestLoadedId: null,
        hasMoreOlder: true,
        isLoadingOlder: false,
      })
    }
    return messages.value.get(peerId)!
  }

  /**
   * Inserts a new message into the appropriate message store and updates conversation data.
   * Handles deduplication and maintains chronological order.
   * @param {Message} msg - The message object to insert.
   */
  function insertMessage(msg: Message) {
    const identityStore = useIdentityStore()
    if (!identityStore.identity) return

    // Determine which peer this message is with
    const peerId = msg.sender === identityStore.identity.peer_id ? msg.recipient : msg.sender
    const store = getOrCreateMessageStore(peerId)

    // Check if message already exists (deduplication)
    if (store.messagesById.has(msg.id)) {
      return
    }

    // Add message to map
    store.messagesById.set(msg.id, msg)

    // Insert into sorted array maintaining order
    const insertIndex = store.sortedIds.findIndex(id => {
      const existing = store.messagesById.get(id)!
      return msg.timestamp < existing.timestamp ||
             (msg.timestamp === existing.timestamp && msg.nonce < existing.nonce)
    })

    if (insertIndex === -1) {
      // Add to end
      store.sortedIds.push(msg.id)
    } else {
      // Insert at correct position
      store.sortedIds.splice(insertIndex, 0, msg.id)
    }

    // Update boundaries
    if (!store.oldestLoadedId || store.sortedIds[0] === msg.id) {
      store.oldestLoadedId = msg.id
    }
    if (!store.newestLoadedId || store.sortedIds[store.sortedIds.length - 1] === msg.id) {
      store.newestLoadedId = msg.id
    }

    // Update conversation last message if this is newer
    const conv = conversations.value.find(c => c.peer_id === peerId)
    if (conv) {
      if (!conv.last_message || msg.timestamp > conv.last_message.timestamp) {
        conv.last_message = msg
      }
    } else {
      // Create new conversation if it doesn't exist
      conversations.value.push({
        peer_id: peerId,
        nickname: null,
        last_message: msg,
        online: false
      })
    }
  }

  /**
   * Fetches the list of conversations from the backend API.
   * Sets loading state and handles error cases.
   * @async
   * @throws {Error} If the API call fails.
   */
  async function fetchConversations() {
    loading.value = true
    error.value = null
    try {
      conversations.value = await listConversations()
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch conversations'
      throw e
    } finally {
      loading.value = false
    }
  }

  /**
   * Fetches the latest messages for a specific peer. Clears existing messages for that peer
   * and reloads with the new data.
   * @async
   * @param {string} peerId - The ID of the peer whose messages are to be fetched.
   * @throws {Error} If the API call fails.
   */
  async function fetchMessages(peerId: string) {
    const store = getOrCreateMessageStore(peerId)

    try {
      const msgs = await getMessages(peerId, 'latest', 50)

      // Clear existing messages for this peer and reload
      store.messagesById.clear()
      store.sortedIds = []
      store.oldestLoadedId = null
      store.newestLoadedId = null
      store.hasMoreOlder = msgs.length === 50

      // Insert all messages
      msgs.forEach(msg => {
        store.messagesById.set(msg.id, msg)
        store.sortedIds.push(msg.id)
      })

      // Sort by timestamp and nonce
      store.sortedIds.sort((a, b) => {
        const msgA = store.messagesById.get(a)!
        const msgB = store.messagesById.get(b)!
        if (msgA.timestamp !== msgB.timestamp) {
          return msgA.timestamp - msgB.timestamp
        }
        return msgA.nonce - msgB.nonce
      })

      if (store.sortedIds.length > 0) {
        store.oldestLoadedId = store.sortedIds[0] || null
        store.newestLoadedId = store.sortedIds[store.sortedIds.length - 1] || null
      }
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch messages'
      throw e
    }
  }

  /**
   * Loads older messages for a specific peer (the active conversation).
   * Appends them to the existing message list while maintaining order.
   * @async
   * @param {string} peerId - The ID of the peer for whom to load older messages.
   * @throws {Error} If the API call fails.
   */
  async function loadOlderMessages(peerId: string) {
    const store = getOrCreateMessageStore(peerId)

    if (!store.hasMoreOlder || store.isLoadingOlder || !store.oldestLoadedId) {
      return
    }

    store.isLoadingOlder = true
    error.value = null

    try {
      const msgs = await getMessages(peerId, 'before', 50, store.oldestLoadedId)

      if (msgs.length === 0) {
        store.hasMoreOlder = false
        return
      }

      // Check if we got fewer messages than requested
      if (msgs.length < 50) {
        store.hasMoreOlder = false
      }

      // Insert all older messages
      msgs.forEach(msg => {
        if (!store.messagesById.has(msg.id)) {
          store.messagesById.set(msg.id, msg)
          store.sortedIds.unshift(msg.id) // Add to beginning
        }
      })

      // Re-sort to ensure correct order
      store.sortedIds.sort((a, b) => {
        const msgA = store.messagesById.get(a)!
        const msgB = store.messagesById.get(b)!
        if (msgA.timestamp !== msgB.timestamp) {
          return msgA.timestamp - msgB.timestamp
        }
        return msgA.nonce - msgB.nonce
      })

      // Update oldest loaded ID
      if (store.sortedIds.length > 0) {
        store.oldestLoadedId = store.sortedIds[0] || null
      }
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to load older messages'
      throw e
    } finally {
      store.isLoadingOlder = false
    }
  }

  /**
   * Sends a message to a specified peer.
   * After successful sending, it inserts the message into the local store.
   * @async
   * @param {string} peerId - The ID of the recipient peer.
   * @param {string} content - The content of the message to send.
   * @throws {Error} If the user identity is not loaded or the API call fails.
   */
  async function sendMessage(peerId: string, content: string) {
    const identityStore = useIdentityStore()
    if (!identityStore.identity) {
      throw new Error('Identity not loaded')
    }

    try {
      // Send message to backend
      const result = await apiSendMessage(peerId, content)

      // Create message object with real ID and add to store
      const newMessage: Message = {
        id: result.id,
        sender: identityStore.identity.peer_id,
        recipient: peerId,
        content,
        timestamp: Date.now(),
        nonce: 0,
        delivery_status: 'Sent'
      }

      insertMessage(newMessage)
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to send message'
      throw e
    }
  }

  /**
   * Sets the currently active conversation by peer ID.
   * @param {string | null} peerId - The ID of the peer to set as active, or null to clear active conversation.
   */
  function setActiveConversation(peerId: string | null) {
    activeConversation.value = peerId
  }

  /**
   * Updates the online status of a specific peer within the conversations list.
   * @param {string} peerId - The ID of the peer whose status to update.
   * @param {boolean} online - The new online status (true for online, false for offline).
   */
  function updatePeerOnlineStatus(peerId: string, online: boolean) {
    const conv = conversations.value.find((c) => c.peer_id === peerId)
    if (conv) {
      conv.online = online
    }
  }

  /**
   * Updates the last message of a conversation if the provided message is newer.
   * @param {Message} msg - The message to consider for updating the last message.
   */
  function updateConversationLastMessage(msg: Message) {
    const identityStore = useIdentityStore()
    if (!identityStore.identity) return

    const peerId = msg.sender === identityStore.identity.peer_id ? msg.recipient : msg.sender
    const conv = conversations.value.find(c => c.peer_id === peerId)

    if (conv) {
      if (!conv.last_message || msg.timestamp > conv.last_message.timestamp) {
        conv.last_message = msg
      }
    }
  }

  /**
   * Updates the delivery status of a specific message across all conversations.
   * This is typically triggered by WebSocket events.
   * @param {string} messageId - The ID of the message to update.
   * @param {import('@/api/types').DeliveryStatus} newStatus - The new delivery status.
   */
  function updateMessageDeliveryStatus(messageId: string, newStatus: import('@/api/types').DeliveryStatus) {
    console.log('[Store] Updating delivery status:', messageId, newStatus)

    // Find message across all conversations
    for (const [peerId, store] of messages.value) {
      const msg = store.messagesById.get(messageId)
      if (msg) {
        console.log('[Store] Found message in', peerId, 'updating from', msg.delivery_status, 'to', newStatus)
        // Create new object for reactivity
        store.messagesById.set(messageId, { ...msg, delivery_status: newStatus })

        // Also update in conversation last message if needed
        const conv = conversations.value.find(c => c.peer_id === peerId)
        if (conv?.last_message?.id === messageId) {
          conv.last_message = { ...msg, delivery_status: newStatus }
        }

        return
      }
    }

    console.log('[Store] Message not found:', messageId, '(might arrive later)')
    // Message not found - it's OK, might arrive later via WebSocket
    // No need to queue, just ignore
  }

  return {
    conversations,
    messages,
    activeConversation,
    loading,
    error,
    sortedConversations,
    activeMessages,
    isLoadingOlderMessages,
    hasMoreOlderMessages,
    fetchConversations,
    fetchMessages,
    loadOlderMessages,
    sendMessage,
    insertMessage,
    setActiveConversation,
    updatePeerOnlineStatus,
    updateConversationLastMessage,
    updateMessageDeliveryStatus,
  }
})
