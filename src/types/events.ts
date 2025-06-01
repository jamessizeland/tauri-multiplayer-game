import { ChatMessage, PeerInfo } from "types";

interface BaseEvent {
  type:
    | "neighborUp"
    | "neighborDown"
    | "errored"
    | "disconnected"
    | "newMessage"
    | "syncFinished"
    | "peerUpdate"
    | "contentReady"
    | "pendingContentReady";
}

/** We have a new, direct neighbor in the swarm membership layer for this topic. */
export interface NeighborUpEvent extends BaseEvent {
  type: "neighborUp";
  nodeId: string;
}

/** We dropped direct neighbor in the swarm membership layer for this topic. */
export interface NeighborDownEvent extends BaseEvent {
  type: "neighborDown";
  nodeId: string;
}

/** Backend reporting an end of stream event.  Not part of the Gossip Events protocol. */
export interface DisconnectedEvent extends BaseEvent {
  type: "disconnected";
}

/** Backend reporting an error event.  Not part of the Gossip Events protocol. */
export interface ErrorEvent extends BaseEvent {
  type: "errored";
  message: string;
}

/** Backend reporting a new message. */
export interface MessageEvent extends BaseEvent {
  type: "newMessage";
  message: ChatMessage;
}

/** Backend reporting it has finished synching the shared activity document.
 *
 * "A set-reconciliation sync finished."
 */
export interface SyncFinishedEvent extends BaseEvent {
  type: "syncFinished";
}

/** Backend reporting a peer has changed. */
export interface PeerUpdateEvent extends BaseEvent {
  type: "peerUpdate";
  info: PeerInfo;
}

/** Backend reporting the content of an entry was downloaded and is now available at the local node */
export interface ContentReadyEvent extends BaseEvent {
  type: "contentReady";
}

/** Backend reporting All pending content is now ready.
 *
 * This event signals that all queued content downloads from the last sync run have either completed or failed.
 * It will only be emitted after a `Self::SyncFinished` event, never before.
 * Receiving this event does not guarantee that all content in the document is available.
 * If blobs failed to download, this event will still be emitted after all operations completed.
 */
export interface PendingContentReadyEvent extends BaseEvent {
  type: "pendingContentReady";
}

/** Gossip Events */
export type ChatEvent =
  | NeighborUpEvent
  | NeighborDownEvent
  | DisconnectedEvent
  | ErrorEvent
  | MessageEvent
  | SyncFinishedEvent
  | PeerUpdateEvent
  | ContentReadyEvent
  | PendingContentReadyEvent;
