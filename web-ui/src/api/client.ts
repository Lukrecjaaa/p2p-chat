import type { Identity, Friend, Conversation, Message } from './types'

const API_BASE = '/api'

export async function getMe(): Promise<Identity> {
  const response = await fetch(`${API_BASE}/me`)
  if (!response.ok) throw new Error('Failed to fetch identity')
  return response.json()
}

export async function listFriends(): Promise<Friend[]> {
  const response = await fetch(`${API_BASE}/friends`)
  if (!response.ok) throw new Error('Failed to fetch friends')
  return response.json()
}

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

export async function listConversations(): Promise<Conversation[]> {
  const response = await fetch(`${API_BASE}/conversations`)
  if (!response.ok) throw new Error('Failed to fetch conversations')
  return response.json()
}

export async function getMessages(peerId: string): Promise<Message[]> {
  const response = await fetch(`${API_BASE}/conversations/${peerId}/messages`)
  if (!response.ok) throw new Error('Failed to fetch messages')
  return response.json()
}

export async function sendMessage(peerId: string, content: string): Promise<{ id: string }> {
  const response = await fetch(`${API_BASE}/conversations/${peerId}/messages`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ content })
  })
  if (!response.ok) throw new Error('Failed to send message')
  return response.json()
}

export async function getOnlinePeers(): Promise<string[]> {
  const response = await fetch(`${API_BASE}/peers/online`)
  if (!response.ok) throw new Error('Failed to fetch online peers')
  return response.json()
}
