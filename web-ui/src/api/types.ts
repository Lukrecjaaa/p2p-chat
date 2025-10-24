export interface Identity {
  peer_id: string
  hpke_public_key: string
}

export interface Friend {
  peer_id: string
  e2e_public_key: string
  nickname: string | null
  online: boolean
}

export interface Message {
  id: string
  sender: string
  recipient: string
  content: string
  timestamp: number
  nonce: number
}

export interface Conversation {
  peer_id: string
  nickname: string | null
  last_message: Message | null
  online: boolean
}

export type WebSocketMessage =
  | {
      type: 'new_message'
      id: string
      sender: string
      recipient: string
      timestamp: number
    }
  | {
      type: 'peer_connected'
      peer_id: string
    }
  | {
      type: 'peer_disconnected'
      peer_id: string
    }
