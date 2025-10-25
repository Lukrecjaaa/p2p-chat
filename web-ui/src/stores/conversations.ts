import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { Conversation, Message } from '@/api/types'
import { listConversations, getMessages, sendMessage as apiSendMessage } from '@/api/client'
import { useIdentityStore } from './identity'

interface MessageStore {
  messagesById: Map<string, Message>
  sortedIds: string[]
  oldestLoadedId: string | null
  newestLoadedId: string | null
  hasMoreOlder: boolean
  isLoadingOlder: boolean
}

export const useConversationsStore = defineStore('conversations', () => {
  const conversations = ref<Conversation[]>([])
  const messages = ref<Map<string, MessageStore>>(new Map())
  const activeConversation = ref<string | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)

  const sortedConversations = computed(() => {
    return [...conversations.value].sort((a, b) => {
      const aTime = a.last_message?.timestamp || 0
      const bTime = b.last_message?.timestamp || 0
      return bTime - aTime
    })
  })

  const activeMessages = computed(() => {
    if (!activeConversation.value) return []
    const store = messages.value.get(activeConversation.value)
    if (!store) return []

    return store.sortedIds.map(id => store.messagesById.get(id)!)
  })

  const isLoadingOlderMessages = computed(() => {
    if (!activeConversation.value) return false
    const store = messages.value.get(activeConversation.value)
    return store?.isLoadingOlder || false
  })

  const hasMoreOlderMessages = computed(() => {
    if (!activeConversation.value) return false
    const store = messages.value.get(activeConversation.value)
    return store?.hasMoreOlder || false
  })

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

  async function sendMessage(peerId: string, content: string) {
    const identityStore = useIdentityStore()
    if (!identityStore.identity) {
      throw new Error('Identity not loaded')
    }

    try {
      const result = await apiSendMessage(peerId, content)

      // Optimistically add message to local state
      const newMessage: Message = {
        id: result.id,
        sender: identityStore.identity.peer_id,
        recipient: peerId,
        content,
        timestamp: Date.now(),
        nonce: 0, // This will be set by backend
        delivery_status: 'Sending'
      }

      insertMessage(newMessage)
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to send message'
      throw e
    }
  }

  function setActiveConversation(peerId: string | null) {
    activeConversation.value = peerId
  }

  function updatePeerOnlineStatus(peerId: string, online: boolean) {
    const conv = conversations.value.find((c) => c.peer_id === peerId)
    if (conv) {
      conv.online = online
    }
  }

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

  function updateMessageDeliveryStatus(messageId: string, newStatus: import('@/api/types').DeliveryStatus) {
    // Find the message across all conversations
    for (const [, store] of messages.value) {
      const msg = store.messagesById.get(messageId)
      if (msg) {
        msg.delivery_status = newStatus
        return
      }
    }
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
