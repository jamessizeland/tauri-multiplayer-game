interface BaseEvent {
  type: "neighborUp" | "neighborDown" | "errored" | "disconnected";
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

/** Gossip Events */
export type ChatEvent =
  | NeighborUpEvent
  | NeighborDownEvent
  | DisconnectedEvent
  | ErrorEvent;
