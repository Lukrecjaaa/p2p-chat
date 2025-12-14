/**
 * @file types.ts
 * @brief This file contains TypeScript type definitions and interfaces for the application's API.
 * It defines the data structures used throughout the frontend for various entities like Identity,
 * Friend, Message, Conversation, and WebSocket messages.
 */
/**
 * Represents the identity of the current user.
 * @interface Identity
 * @property {string} peer_id - The unique identifier for the peer.
 * @property {string} hpke_public_key - The Hybrid Public Key Encryption public key.
 */
export interface Identity {
  peer_id: string
  hpke_public_key: string
}

/**
 * Represents a friend in the chat application.
 * @interface Friend
 * @property {string} peer_id - The unique identifier for the friend's peer.
 * @property {string} e2e_public_key - The end-to-end encryption public key of the friend.
 * @property {string | null} nickname - The display name of the friend, or null if not set.
 * @property {boolean} online - Indicates if the friend is currently online.
 */
export interface Friend {
  peer_id: string
  e2e_public_key: string
  nickname: string | null
  online: boolean
}

/**
 * Represents the delivery status of a message.
 * @typedef {'Sending' | 'Sent' | 'Delivered' | 'Read'} DeliveryStatus
 */
export type DeliveryStatus = 'Sending' | 'Sent' | 'Delivered' | 'Read'

/**
 * Represents a chat message.
 * @interface Message
 * @property {string} id - The unique identifier of the message.
 * @property {string} sender - The peer ID of the message sender.
 * @property {string} recipient - The peer ID of the message recipient.
 * @property {string} content - The content of the message.
 * @property {number} timestamp - The timestamp when the message was sent (Unix epoch milliseconds).
 * @property {number} nonce - A cryptographic nonce for the message.
 * @property {DeliveryStatus} delivery_status - The current delivery status of the message.
 */
export interface Message {
  id: string
  sender: string
  recipient: string
  content: string
  timestamp: number
  nonce: number
  delivery_status: DeliveryStatus
}

/**
 * Represents a conversation with a peer.
 * @interface Conversation
 * @property {string} peer_id - The unique identifier for the peer in the conversation.
 * @property {string | null} nickname - The display name of the peer, or null if not set.
 * @property {Message | null} last_message - The last message exchanged in the conversation, or null if no messages.
 * @property {boolean} online - Indicates if the peer is currently online.
 */
export interface Conversation {
  peer_id: string
  nickname: string | null
  last_message: Message | null
  online: boolean
}

/**
 * Represents the types of messages that can be received over a WebSocket connection.
 * @typedef {object} WebSocketMessage
 * @property {'new_message'} type - Indicates a new message has been received.
 * @property {string} id - The unique identifier of the new message.
 * @property {string} sender - The peer ID of the message sender.
 * @property {string} recipient - The peer ID of the message recipient.
 * @property {string} content - The content of the new message.
 * @property {number} timestamp - The timestamp when the message was sent.
 * @property {number} nonce - A cryptographic nonce.
 * @property {DeliveryStatus} delivery_status - The delivery status of the new message.
 *
 * @property {'peer_connected'} type - Indicates a peer has connected.
 * @property {string} peer_id - The peer ID of the connected peer.
 *
 * @property {'peer_disconnected'} type - Indicates a peer has disconnected.
 * @property {string} peer_id - The peer ID of the disconnected peer.
 *
 * @property {'delivery_status_update'} type - Indicates an update to a message's delivery status.
 * @property {string} message_id - The ID of the message whose status is being updated.
 * @property {DeliveryStatus} new_status - The new delivery status of the message.
 */
export type WebSocketMessage =
  | {
      type: 'new_message'
      id: string
      sender: string
      recipient: string
      content: string
      timestamp: number
      nonce: number
      delivery_status: DeliveryStatus
    }
  | {
      type: 'peer_connected'
      peer_id: string
    }
  | {
      type: 'peer_disconnected'
      peer_id: string
    }
  | {
      type: 'delivery_status_update'
      message_id: string
      new_status: DeliveryStatus
    }
