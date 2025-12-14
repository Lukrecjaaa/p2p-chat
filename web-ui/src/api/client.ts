/**
 * @file client.ts
 * @brief This file provides a client for interacting with the application's backend API.
 * It includes functions for fetching identity, managing friends, conversations, messages,
 * and system status, all using standard Fetch API.
 */
import type { Identity, Friend, Conversation, Message } from './types'

/**
 * @constant {string} API_BASE - The base URL for the API endpoints.
 */
const API_BASE = '/api'

/**
 * Fetches the current user's identity from the API.
 * @returns {Promise<Identity>} A promise that resolves to the current user's Identity.
 * @throws {Error} If the API call fails.
 */
export async function getMe(): Promise<Identity> {
  const response = await fetch(`${API_BASE}/me`)
  if (!response.ok) throw new Error('Failed to fetch identity')
  return response.json()
}

/**
 * Fetches a list of all friends from the API.
 * @returns {Promise<Friend[]>} A promise that resolves to an array of Friend objects.
 * @throws {Error} If the API call fails.
 */
export async function listFriends(): Promise<Friend[]> {
  const response = await fetch(`${API_BASE}/friends`)
  if (!response.ok) throw new Error('Failed to fetch friends')
  return response.json()
}

/**
 * Adds a new friend to the system.
 * @param {string} peerId - The Peer ID of the friend to add.
 * @param {string} publicKey - The public key of the friend.
 * @param {string} [nickname] - An optional nickname for the friend.
 * @returns {Promise<void>} A promise that resolves when the friend is successfully added.
 * @throws {Error} If the API call fails.
 */
export async function addFriend(
  peerId: string,
  publicKey: string,
  nickname?: string
): Promise<void> {
  const response = await fetch(`${API_BASE}/friends`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      peer_id: peerId,
      e2e_public_key: publicKey,
      nickname
    })
  })
  if (!response.ok) throw new Error('Failed to add friend')
}

/**
 * Fetches a list of all conversations from the API.
 * @returns {Promise<Conversation[]>} A promise that resolves to an array of Conversation objects.
 * @throws {Error} If the API call fails.
 */
export async function listConversations(): Promise<Conversation[]> {
  const response = await fetch(`${API_BASE}/conversations`)
  if (!response.ok) throw new Error('Failed to fetch conversations')
  return response.json()
}

/**
 * Fetches messages for a specific peer.
 * @param {string} peerId - The Peer ID of the conversation partner.
 * @param {'latest' | 'before' | 'after'} [mode='latest'] - The mode of fetching messages: 'latest', 'before' a reference message, or 'after' a reference message.
 * @param {number} [limit=50] - The maximum number of messages to fetch.
 * @param {string} [referenceId] - The ID of the reference message when mode is 'before' or 'after'.
 * @returns {Promise<Message[]>} A promise that resolves to an array of Message objects.
 * @throws {Error} If the API call fails.
 */
export async function getMessages(
  peerId: string,
  mode: 'latest' | 'before' | 'after' = 'latest',
  limit: number = 50,
  referenceId?: string
): Promise<Message[]> {
  const params = new URLSearchParams()
  params.set('mode', mode)
  params.set('limit', limit.toString())

  if (mode === 'before' && referenceId) {
    params.set('before_id', referenceId)
  } else if (mode === 'after' && referenceId) {
    params.set('after_id', referenceId)
  }

  const response = await fetch(`${API_BASE}/conversations/${peerId}/messages?${params}`)
  if (!response.ok) throw new Error('Failed to fetch messages')
  return response.json()
}

/**
 * Sends a message to a specific peer.
 * @param {string} peerId - The Peer ID of the recipient.
 * @param {string} content - The content of the message.
 * @returns {Promise<{ id: string }>} A promise that resolves to an object containing the ID of the sent message.
 * @throws {Error} If the API call fails.
 */
export async function sendMessage(
  peerId: string,
  content: string
): Promise<{ id: string }> {
  const response = await fetch(`${API_BASE}/conversations/${peerId}/messages`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ content })
  })
  if (!response.ok) throw new Error('Failed to send message')
  return response.json()
}

/**
 * Marks a specific message as read.
 * @param {string} messageId - The ID of the message to mark as read.
 * @returns {Promise<void>} A promise that resolves when the message is successfully marked as read.
 * @throws {Error} If the API call fails.
 */
export async function markMessageRead(messageId: string): Promise<void> {
  const response = await fetch(`${API_BASE}/messages/${messageId}/read`, {
    method: 'POST'
  })
  if (!response.ok) throw new Error('Failed to mark message as read')
}

/**
 * Fetches a list of currently online peers.
 * @returns {Promise<string[]>} A promise that resolves to an array of Peer IDs of online peers.
 * @throws {Error} If the API call fails.
 */
export async function getOnlinePeers(): Promise<string[]> {
  const response = await fetch(`${API_BASE}/peers/online`)
  if (!response.ok) throw new Error('Failed to fetch online peers')
  return response.json()
}

/**
 * @interface SystemStatus
 * @property {number} connected_peers - The number of currently connected peers.
 * @property {number} known_mailboxes - The number of known mailboxes.
 * @property {number} pending_messages - The number of pending messages.
 */
export interface SystemStatus {
  connected_peers: number
  known_mailboxes: number
  pending_messages: number
}

/**
 * Fetches the current system status from the API.
 * @returns {Promise<SystemStatus>} A promise that resolves to a SystemStatus object.
 * @throws {Error} If the API call fails.
 */
export async function getSystemStatus(): Promise<SystemStatus> {
  const response = await fetch(`${API_BASE}/system/status`)
  if (!response.ok) throw new Error('Failed to fetch system status')
  return response.json()
}
